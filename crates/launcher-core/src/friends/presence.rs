//! Online status (ARCHITECTURE K66): an open launcher with friends on calls
//! `heartbeat()` every [`HEARTBEAT_SECS`]; friends count as online while the
//! last one is younger than 90 s (see `my_friends()` in phase13.sql).

use std::sync::atomic::Ordering;

use serde_json::json;

use super::FriendsClient;
use crate::error::Result;

pub const HEARTBEAT_SECS: u64 = 60;

impl FriendsClient {
    /// Marks the caller online. The outcome is remembered: a failed
    /// heartbeat (no internet, server down) means friends see us offline.
    pub async fn heartbeat(&self) -> Result<()> {
        if !self.is_enabled().await {
            self.online.store(false, Ordering::Relaxed);
            return Ok(());
        }
        let r = self.rpc_void("heartbeat", json!({})).await;
        self.online.store(r.is_ok(), Ordering::Relaxed);
        r
    }

    /// Tells friends we left (on exit). Without it the status times out.
    pub async fn go_offline(&self) -> Result<()> {
        let was = self.online.swap(false, Ordering::Relaxed);
        if !was || !self.is_enabled().await {
            return Ok(());
        }
        self.rpc_void("go_offline", json!({})).await
    }

    /// Whether friends currently see us online (last heartbeat succeeded).
    pub fn is_online(&self) -> bool {
        self.online.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::FriendsClient;
    use super::super::tests::{ctx, signed_in};

    #[tokio::test]
    async fn heartbeat_tracks_online_state() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/heartbeat"))
            .respond_with(ResponseTemplate::new(204))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/heartbeat"))
            .respond_with(ResponseTemplate::new(500).set_body_json(json!({"message": "down"})))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/go_offline"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert!(!c.is_online());
        c.heartbeat().await.unwrap();
        assert!(c.is_online());
        c.go_offline().await.unwrap();
        assert!(!c.is_online());
        // Already offline: nothing is sent (go_offline expects one call).
        c.go_offline().await.unwrap();
        // A failing heartbeat means offline.
        assert!(c.heartbeat().await.is_err());
        assert!(!c.is_online());
    }

    #[tokio::test]
    async fn disabled_client_never_calls_the_server() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        c.heartbeat().await.unwrap();
        c.go_offline().await.unwrap();
        assert!(!c.is_online());
        assert!(server.received_requests().await.unwrap().is_empty());
    }
}
