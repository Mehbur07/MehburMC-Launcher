//! Voice calls with a friend or a group (K86). Audio flows peer to peer in
//! the webview (WebRTC); the server (`supabase/phase24.sql`) only tracks who
//! is in which call and relays the connection set-up messages. TURN
//! credentials come from the `turn-credentials` Edge Function, which keeps
//! the Cloudflare secret on the server.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::{Duration, Instant};

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use super::{FriendsClient, check_uuid};
use crate::error::{CoreError, Result};

/// Codes the call RPCs raise; each has an `errors.calls.*` translation.
pub const CALL_ERRORS: &[&str] = &[
    "calls.invalid",
    "calls.notFound",
    "calls.ended",
    "calls.turnUnavailable",
];

/// Signals per `call_signal` request (matches the database).
pub const MAX_SIGNALS_PER_BATCH: usize = 50;
/// Longest signal payload (matches the database check).
pub const MAX_SIGNAL_CHARS: usize = 16000;

/// Always offered, with or without TURN.
const DEFAULT_STUN: &[&str] = &[
    "stun:stun.cloudflare.com:3478",
    "stun:stun.l.google.com:19302",
];
/// Credentials are renewed this long before they expire.
const ICE_RENEW_EARLY: Duration = Duration::from_secs(3600);
/// Retry after a failed or empty credentials request.
const ICE_RETRY: Duration = Duration::from_secs(600);
const MAX_ICE_SERVERS: usize = 8;
const MAX_ICE_URLS: usize = 12;
const MAX_ICE_FIELD: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CallMemberState {
    Ringing,
    Joined,
    Left,
    Declined,
    Missed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CallMember {
    pub id: String,
    pub display_name: String,
    /// The caller's own name for this member (K85).
    #[serde(default)]
    pub nickname: Option<String>,
    pub state: CallMemberState,
    /// Set once joined; the later joiner of each pair sends the offer.
    #[serde(default)]
    pub joined_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Call {
    #[ts(type = "number")]
    pub id: i64,
    /// `None` for a call between two friends.
    #[ts(type = "number | null")]
    pub group_id: Option<i64>,
    #[serde(default)]
    pub created_by: Option<String>,
    pub created_at: String,
    /// The caller's own state; `None` for a group call they were never in.
    #[serde(default)]
    pub my_state: Option<CallMemberState>,
    /// Members ringing or in the call, in joining order.
    #[serde(default)]
    pub members: Vec<CallMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SignalKind {
    Offer,
    Answer,
    Ice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CallSignal {
    #[ts(type = "number")]
    pub id: i64,
    #[ts(type = "number")]
    pub call_id: i64,
    pub sender: String,
    pub kind: SignalKind,
    pub payload: String,
}

/// What one poll returns: live calls and the signals addressed to us.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CallPoll {
    #[serde(default)]
    pub calls: Vec<Call>,
    #[serde(default)]
    pub signals: Vec<CallSignal>,
}

/// A signal the webview sends to one member of the call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutgoingSignal {
    pub recipient: String,
    pub kind: SignalKind,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IceConfig {
    pub ice_servers: Vec<IceServer>,
    /// A TURN relay is available (the "hide my IP" setting can apply).
    pub relay: bool,
}

/// Call state kept by the client between requests.
#[derive(Default)]
pub(crate) struct CallCache {
    /// The call this launcher is in; 0 for none.
    active: AtomicI64,
    ice: tokio::sync::Mutex<Option<(IceConfig, Instant)>>,
}

fn is_ice_url(u: &str) -> bool {
    (u.starts_with("stun:") || u.starts_with("turn:") || u.starts_with("turns:"))
        && u.len() <= MAX_ICE_FIELD
        && !u.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Keeps only well-formed STUN/TURN entries from the Edge Function.
fn clean_ice(v: &Value) -> Vec<IceServer> {
    let list = match &v["iceServers"] {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        _ => Vec::new(),
    };
    let field = |s: &Value, k: &str| {
        s[k].as_str()
            .filter(|x| !x.is_empty() && x.len() <= MAX_ICE_FIELD)
            .map(str::to_owned)
    };
    list.iter()
        .filter_map(|s| {
            let urls: Vec<String> = match &s["urls"] {
                Value::String(u) => vec![u.clone()],
                Value::Array(a) => a
                    .iter()
                    .filter_map(|u| u.as_str().map(str::to_owned))
                    .collect(),
                _ => Vec::new(),
            }
            .into_iter()
            .filter(|u| is_ice_url(u))
            .take(MAX_ICE_URLS)
            .collect();
            let (username, credential) = (field(s, "username"), field(s, "credential"));
            // TURN needs both; a TURN entry without them is useless.
            let urls: Vec<String> = urls
                .into_iter()
                .filter(|u| u.starts_with("stun:") || (username.is_some() && credential.is_some()))
                .collect();
            (!urls.is_empty()).then_some(IceServer {
                urls,
                username,
                credential,
            })
        })
        .take(MAX_ICE_SERVERS)
        .collect()
}

fn with_default_stun(mut servers: Vec<IceServer>) -> IceConfig {
    let relay = servers
        .iter()
        .any(|s| s.credential.is_some() && s.urls.iter().any(|u| u.starts_with("turn")));
    servers.push(IceServer {
        urls: DEFAULT_STUN.iter().map(|u| (*u).to_owned()).collect(),
        username: None,
        credential: None,
    });
    IceConfig {
        ice_servers: servers,
        relay,
    }
}

fn check_signals(signals: &[OutgoingSignal]) -> Result<()> {
    if signals.is_empty() || signals.len() > MAX_SIGNALS_PER_BATCH {
        return Err(CoreError::Friends("calls.invalid"));
    }
    for s in signals {
        check_uuid(&s.recipient).map_err(|_| CoreError::Friends("calls.invalid"))?;
        let n = s.payload.chars().count();
        if n == 0 || n > MAX_SIGNAL_CHARS {
            return Err(CoreError::Friends("calls.invalid"));
        }
    }
    Ok(())
}

impl FriendsClient {
    /// Calls a friend or a group (exactly one of the two), or joins its live
    /// call. Returns the call id; any other call of ours is left.
    pub async fn start_call(&self, friend: Option<&str>, group: Option<i64>) -> Result<i64> {
        let args = match (friend, group) {
            (Some(f), None) => {
                check_uuid(f)?;
                json!({ "p_friend": f })
            }
            (None, Some(g)) => json!({ "p_group": g }),
            _ => return Err(CoreError::Friends("calls.invalid")),
        };
        let id: i64 = self.rpc("call_start", args).await?;
        self.calls.active.store(id, Ordering::SeqCst);
        Ok(id)
    }

    /// Answers a ringing call or joins a group's live call.
    pub async fn join_call(&self, call: i64) -> Result<()> {
        self.rpc_void("call_join", json!({ "p_call": call }))
            .await?;
        self.calls.active.store(call, Ordering::SeqCst);
        Ok(())
    }

    /// Leaves a call, or declines it while it rings.
    pub async fn leave_call(&self, call: i64) -> Result<()> {
        let _ = self
            .calls
            .active
            .compare_exchange(call, 0, Ordering::SeqCst, Ordering::SeqCst);
        self.rpc_void("call_leave", json!({ "p_call": call })).await
    }

    /// Leaves the active call, if any (on exit).
    pub async fn leave_active_call(&self) -> Result<()> {
        match self.active_call() {
            Some(id) => self.leave_call(id).await,
            None => Ok(()),
        }
    }

    /// The call this launcher is in.
    pub fn active_call(&self) -> Option<i64> {
        Some(self.calls.active.load(Ordering::SeqCst)).filter(|id| *id != 0)
    }

    /// Delivers set-up messages; returns how many reached someone still in
    /// the call.
    pub async fn send_signals(&self, call: i64, signals: &[OutgoingSignal]) -> Result<u32> {
        check_signals(signals)?;
        self.rpc(
            "call_signal",
            json!({ "p_call": call, "p_signals": signals }),
        )
        .await
    }

    /// Live calls and new signals; signals up to `after` are acknowledged
    /// (the server deletes them).
    pub async fn poll_calls(&self, after: i64) -> Result<CallPoll> {
        let poll: CallPoll = self.rpc("call_poll", json!({ "p_after": after })).await?;
        // The server dropped us (time-out, removed from the group, call over).
        if let Some(active) = self.active_call() {
            let still_in = poll
                .calls
                .iter()
                .any(|c| c.id == active && c.my_state == Some(CallMemberState::Joined));
            if !still_in {
                let _ = self.calls.active.compare_exchange(
                    active,
                    0,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                );
            }
        }
        Ok(poll)
    }

    /// STUN servers plus TURN when the Edge Function is set up. Never fails:
    /// without TURN the list holds the public STUN servers only.
    pub async fn ice_config(&self) -> IceConfig {
        let mut cache = self.calls.ice.lock().await;
        if let Some((cfg, until)) = cache.as_ref()
            && Instant::now() < *until
        {
            return cfg.clone();
        }
        let (servers, valid_for) = match self
            .json::<Value>(
                Method::POST,
                "/functions/v1/turn-credentials",
                Some(&json!({})),
                &[],
            )
            .await
        {
            Ok(v) => {
                let servers = clean_ice(&v);
                let ttl = Duration::from_secs(v["ttl"].as_u64().unwrap_or(0));
                let valid = if servers.is_empty() || ttl <= ICE_RENEW_EARLY {
                    ICE_RETRY
                } else {
                    ttl - ICE_RENEW_EARLY
                };
                (servers, valid)
            }
            Err(e) => {
                tracing::info!(error = %e.detail(), "no TURN credentials; calls use STUN only");
                (Vec::new(), ICE_RETRY)
            }
        };
        let cfg = with_default_stun(servers);
        *cache = Some((cfg.clone(), Instant::now() + valid_for));
        cfg
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_json, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{ME, signed_in};
    use super::*;

    const B: &str = "22222222-2222-2222-2222-222222222222";

    fn sig(kind: SignalKind, payload: &str) -> OutgoingSignal {
        OutgoingSignal {
            recipient: B.into(),
            kind,
            payload: payload.into(),
        }
    }

    #[tokio::test]
    async fn validates_before_the_server() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let code = |r: Result<i64>| r.unwrap_err().code();
        assert_eq!(code(c.start_call(None, None).await), "calls.invalid");
        assert_eq!(code(c.start_call(Some(B), Some(1)).await), "calls.invalid");
        assert_eq!(
            code(c.start_call(Some("x"), None).await),
            "friends.notFriends"
        );
        let scode = |r: Result<u32>| r.unwrap_err().code();
        assert_eq!(scode(c.send_signals(1, &[]).await), "calls.invalid");
        let many: Vec<_> = (0..51).map(|_| sig(SignalKind::Ice, "c")).collect();
        assert_eq!(scode(c.send_signals(1, &many).await), "calls.invalid");
        assert_eq!(
            scode(c.send_signals(1, &[sig(SignalKind::Offer, "")]).await),
            "calls.invalid"
        );
        assert_eq!(
            scode(
                c.send_signals(1, &[sig(SignalKind::Offer, &"x".repeat(16001))])
                    .await
            ),
            "calls.invalid"
        );
        let bad = OutgoingSignal {
            recipient: "nope".into(),
            ..sig(SignalKind::Ice, "c")
        };
        assert_eq!(scode(c.send_signals(1, &[bad]).await), "calls.invalid");
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn tracks_the_active_call() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/call_start"))
            .and(body_json(json!({ "p_friend": B })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(7)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/call_join"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/call_leave"))
            .and(body_json(json!({ "p_call": 9 })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/call_poll"))
            .and(body_json(json!({ "p_after": 0 })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "calls": [{ "id": 9, "groupId": 3, "createdBy": B, "createdAt": "t",
                            "myState": "left", "members": [] }],
                "signals": []
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        assert_eq!(c.active_call(), None);
        assert_eq!(c.start_call(Some(B), None).await.unwrap(), 7);
        assert_eq!(c.active_call(), Some(7));
        c.join_call(9).await.unwrap();
        assert_eq!(c.active_call(), Some(9));
        // The server reports us out of call 9 (timed out): no longer active.
        let poll = c.poll_calls(0).await.unwrap();
        assert_eq!(poll.calls[0].group_id, Some(3));
        assert_eq!(c.active_call(), None);
        c.join_call(9).await.unwrap();
        c.leave_active_call().await.unwrap();
        assert_eq!(c.active_call(), None);
    }

    #[tokio::test]
    async fn polls_calls_and_signals() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/call_poll"))
            .and(body_json(json!({ "p_after": 41 })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "calls": [{
                    "id": 5, "groupId": null, "createdBy": B, "createdAt": "t",
                    "myState": "ringing",
                    "members": [
                        { "id": B, "displayName": "Bob", "nickname": "Bobby",
                          "state": "joined", "joinedAt": "t1" },
                        { "id": ME, "displayName": "Me", "nickname": null,
                          "state": "ringing", "joinedAt": null }
                    ]
                }],
                "signals": [{ "id": 42, "callId": 5, "sender": B, "kind": "offer", "payload": "sdp" }]
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/call_signal"))
            .and(body_json(json!({ "p_call": 5, "p_signals": [
                { "recipient": B, "kind": "answer", "payload": "sdp2" }
            ] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(1)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let p = c.poll_calls(41).await.unwrap();
        assert_eq!(p.calls[0].my_state, Some(CallMemberState::Ringing));
        assert_eq!(p.calls[0].group_id, None);
        assert_eq!(p.calls[0].members[0].nickname.as_deref(), Some("Bobby"));
        assert_eq!(p.signals[0].kind, SignalKind::Offer);
        assert_eq!(
            c.send_signals(5, &[sig(SignalKind::Answer, "sdp2")])
                .await
                .unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn ice_config_uses_turn_when_available_and_caches_it() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/functions/v1/turn-credentials"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "ttl": 86400,
                "iceServers": [
                    { "urls": ["stun:stun.cloudflare.com:3478"] },
                    { "urls": ["turn:turn.cloudflare.com:3478?transport=udp",
                               "turns:turn.cloudflare.com:443?transport=tcp",
                               "https://evil.example/x", "turn:bad host"],
                      "username": "u", "credential": "p" },
                    { "urls": ["turn:no-credentials.example:3478"] }
                ]
            })))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let cfg = c.ice_config().await;
        assert!(cfg.relay);
        assert_eq!(cfg.ice_servers.len(), 3, "{cfg:?}");
        assert_eq!(cfg.ice_servers[1].urls.len(), 2);
        assert_eq!(cfg.ice_servers[1].credential.as_deref(), Some("p"));
        assert!(cfg.ice_servers[2].urls[0].starts_with("stun:"));
        // Second call is served from the cache (`expect(1)` above).
        assert_eq!(c.ice_config().await, cfg);
    }

    #[tokio::test]
    async fn ice_config_falls_back_to_stun() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/functions/v1/turn-credentials"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Function not found"))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let cfg = c.ice_config().await;
        assert!(!cfg.relay);
        assert_eq!(cfg.ice_servers.len(), 1);
        assert!(
            cfg.ice_servers[0]
                .urls
                .iter()
                .all(|u| u.starts_with("stun:"))
        );
    }

    #[test]
    fn single_object_ice_servers_are_accepted() {
        let v = json!({ "iceServers": { "urls": "turn:t.example:3478", "username": "u", "credential": "p" } });
        let s = clean_ice(&v);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].urls, vec!["turn:t.example:3478".to_owned()]);
    }
}
