//! Account names unique across all MehburMC users (ARCHITECTURE K67). Names
//! are reserved in `account_names` by the installation's anonymous identity
//! (created on first use, friends on or off). Releases that fail offline are
//! queued in `launcher/pending-releases.json` and retried at startup.

use std::collections::{BTreeSet, HashMap};

use serde::Deserialize;
use serde_json::json;

use super::FriendsClient;
use crate::error::{CoreError, Result};
use crate::fsutil::write_json_atomic;

/// Codes the name functions raise (`raise exception '<code>'`).
pub const NAME_ERRORS: &[&str] = &[
    "account.nameTaken",
    "account.nameLimit",
    "account.nameInvalid",
];

/// Outcome of claiming an existing local name at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimResult {
    Ok,
    /// Someone else has it.
    Taken,
    /// This installation already holds the maximum number of names.
    Limit,
    Invalid,
}

/// Maps transport problems to messages about the name check itself.
fn name_error(e: CoreError) -> CoreError {
    match e.code() {
        c if c.starts_with("net.") => CoreError::Friends("account.nameCheckOffline"),
        "friends.unavailable" => CoreError::Friends("account.nameServiceUnavailable"),
        _ => match e {
            CoreError::Friends("account.nameTaken") => CoreError::AccountNameTaken(String::new()),
            other => other,
        },
    }
}

impl FriendsClient {
    /// Reserves `name` for this installation (idempotent).
    pub async fn claim_name(&self, name: &str) -> Result<()> {
        self.ensure_identity().await.map_err(name_error)?;
        self.rpc_void("claim_name", json!({ "name": name }))
            .await
            .map_err(name_error)
    }

    /// Moves the reservation from `old` to `new` in one step.
    pub async fn rename_name(&self, old: &str, new: &str) -> Result<()> {
        self.ensure_identity().await.map_err(name_error)?;
        self.rpc_void("rename_name", json!({ "old_name": old, "new_name": new }))
            .await
            .map_err(name_error)
    }

    /// Lets `name` go. Offline failures are queued and retried later.
    pub async fn release_name(&self, name: &str) {
        if !self.has_identity().await {
            return;
        }
        if let Err(e) = self.rpc_void("release_name", json!({ "name": name })).await {
            tracing::debug!(error = %e.detail(), "name release queued");
            let mut pending = self.pending_releases();
            pending.insert(name.to_owned());
            self.save_pending_releases(&pending);
        }
    }

    fn pending_releases(&self) -> BTreeSet<String> {
        std::fs::read(self.ctx().paths.pending_releases_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save_pending_releases(&self, names: &BTreeSet<String>) {
        let path = self.ctx().paths.pending_releases_file();
        let r = if names.is_empty() {
            match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(CoreError::io(&path, e)),
                _ => Ok(()),
            }
        } else {
            write_json_atomic(&path, names)
        };
        if let Err(e) = r {
            tracing::warn!(error = %e.detail(), "could not save pending name releases");
        }
    }

    /// Startup: releases queued names (except ones still in use locally),
    /// then claims every local name. Returns the outcome per name.
    pub async fn sync_names(&self, local: &[String]) -> Result<HashMap<String, ClaimResult>> {
        let in_use: BTreeSet<String> = local.iter().map(|n| n.to_ascii_lowercase()).collect();
        let mut pending = self.pending_releases();
        pending.retain(|n| !in_use.contains(&n.to_ascii_lowercase()));
        if !pending.is_empty() && self.has_identity().await {
            let mut left = BTreeSet::new();
            for n in &pending {
                if self
                    .rpc_void("release_name", json!({ "name": n }))
                    .await
                    .is_err()
                {
                    left.insert(n.clone());
                }
            }
            pending = left;
        }
        self.save_pending_releases(&pending);
        if local.is_empty() {
            return Ok(HashMap::new());
        }

        #[derive(Deserialize)]
        struct Row {
            name: String,
            result: String,
        }
        self.ensure_identity().await.map_err(name_error)?;
        let rows: Vec<Row> = self
            .rpc("claim_names", json!({ "names": local }))
            .await
            .map_err(name_error)?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let result = match r.result.as_str() {
                    "ok" => ClaimResult::Ok,
                    "limit" => ClaimResult::Limit,
                    "invalid" => ClaimResult::Invalid,
                    _ => ClaimResult::Taken,
                };
                (r.name, result)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::FriendsClient;
    use super::super::tests::{auth_body, ctx, signed_in};
    use super::ClaimResult;

    fn raise(code: &str) -> ResponseTemplate {
        ResponseTemplate::new(400).set_body_json(json!({ "code": "P0001", "message": code }))
    }

    #[tokio::test]
    async fn claim_creates_an_identity_without_turning_friends_on() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/signup"))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_body("tok", 3600)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_name"))
            .and(body_json(json!({ "name": "Mehbur" })))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_name"))
            .and(body_json(json!({ "name": "Notch" })))
            .respond_with(raise("account.nameTaken"))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        c.claim_name("Mehbur").await.unwrap();
        assert!(c.has_identity().await);
        assert!(!c.is_enabled().await);
        let e = c.claim_name("Notch").await.unwrap_err();
        assert_eq!(e.code(), "account.nameTaken");
        // Reloaded from disk: still an identity with friends off.
        let again = FriendsClient::new(ctx(&tmp, &server));
        assert!(again.has_identity().await && !again.is_enabled().await);
    }

    #[tokio::test]
    async fn offline_and_disabled_service_have_their_own_messages() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/signup"))
            .respond_with(ResponseTemplate::new(422).set_body_json(json!({
                "code": 422, "error_code": "anonymous_provider_disabled",
                "msg": "Anonymous sign-ins are disabled"
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        let e = c.claim_name("Mehbur").await.unwrap_err();
        assert_eq!(e.code(), "account.nameServiceUnavailable");

        // Nothing listens on this port: a network error.
        let mut offline = ctx(&tmp, &server);
        offline.endpoints.friends = "http://127.0.0.1:9".into();
        let c = FriendsClient::new(offline);
        let e = c.claim_name("Mehbur").await.unwrap_err();
        assert_eq!(e.code(), "account.nameCheckOffline");
    }

    #[tokio::test]
    async fn failed_releases_are_retried_on_sync() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/release_name"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/release_name"))
            .and(body_json(json!({ "name": "OldName" })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/claim_names"))
            .and(body_json(json!({ "names": ["Mehbur", "Steve"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "name": "Mehbur", "result": "ok" },
                { "name": "Steve", "result": "taken" }
            ])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.release_name("OldName").await;
        let file = c.ctx().paths.pending_releases_file();
        assert!(file.exists());

        let out = c
            .sync_names(&["Mehbur".into(), "Steve".into()])
            .await
            .unwrap();
        assert_eq!(out["Mehbur"], ClaimResult::Ok);
        assert_eq!(out["Steve"], ClaimResult::Taken);
        assert!(!file.exists());
    }
}
