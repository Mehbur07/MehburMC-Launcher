//! Friends: friend codes, chat and shared mod lists on the MehburMC Supabase
//! project (ARCHITECTURE K64). Server rules live in `supabase/schema.sql`;
//! this client only holds an anonymous identity in `launcher/friends.json`.

pub mod account;
pub mod admin;
pub mod avatar;
pub mod library;
pub mod names;
pub mod presence;
pub mod share;
pub mod textures;

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use ts_rs::TS;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::fsutil::write_json_atomic;

/// Public "anon" key of the Supabase project. Safe to ship: every table is
/// protected by row-level security, this key only identifies the project.
pub const ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InJjYWlmd3NpdXRveGlscGtjZWh0Iiwicm9sZSI6ImFub24iLCJpYXQiOjE3OTEyMjI3ODEsImV4cCI6MjEwNjc5ODc4MX0.tH-dLTbuGXWnV0Mq4ZoDDrbFN3CEfW-yKS6SCKXYu7E";

/// Codes the server raises (`raise exception '<code>'`) or the client
/// returns; each has an `errors.friends.*` translation.
pub const FRIEND_ERRORS: &[&str] = &[
    "friends.disabled",
    "friends.unavailable",
    "friends.codeNotFound",
    "friends.self",
    "friends.alreadyFriends",
    "friends.notFriends",
    "friends.rateLimited",
    "friends.messageInvalid",
    "friends.listNotFound",
    "friends.badItem",
];

/// Longest chat message (matches the database check).
pub const MAX_MESSAGE_CHARS: usize = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    user_id: String,
    access_token: String,
    refresh_token: String,
    /// Unix seconds.
    expires_at: u64,
    /// The identity also reserves account names (K67), so it exists with
    /// friends off. Sessions saved before that were always friends-on.
    #[serde(default = "yes")]
    friends_enabled: bool,
    /// Email of the MehburMC account (K73); `None` for anonymous identities.
    #[serde(default)]
    email: Option<String>,
    /// Anonymous identity of launchers before K73 (upgraded on sign-up).
    #[serde(default = "yes")]
    anonymous: bool,
    /// Last rank/ban the server reported, for offline starts.
    #[serde(default)]
    account: account::AccountCache,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Profile {
    pub id: String,
    pub friend_code: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FriendStatus {
    Pending,
    Accepted,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Friend {
    pub id: String,
    pub friend_code: String,
    pub display_name: String,
    pub status: FriendStatus,
    /// For pending requests: sent to me (true) or by me (false).
    pub incoming: bool,
    #[ts(type = "number")]
    pub request_id: i64,
    #[ts(type = "number")]
    pub unread: i64,
    /// Profile photo as a `data:` URI (accepted friends only).
    pub avatar: Option<String>,
    /// Launcher open right now (accepted friends only).
    pub online: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChatMessage {
    #[ts(type = "number")]
    pub id: i64,
    pub sender: String,
    pub recipient: String,
    pub body: String,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FriendsStatus {
    /// The user turned friends on (an identity exists).
    pub enabled: bool,
    pub profile: Option<Profile>,
    /// Friends see us online (the last heartbeat succeeded).
    pub online: bool,
}

/// Row returned by `my_friends()` (snake_case from SQL).
#[derive(Deserialize)]
struct FriendRow {
    id: String,
    friend_code: String,
    display_name: String,
    status: FriendStatus,
    incoming: bool,
    request_id: i64,
    unread: i64,
    #[serde(default)]
    avatar_sha1: Option<String>,
    #[serde(default)]
    online: bool,
}

#[derive(Deserialize)]
struct ProfileRow {
    id: String,
    friend_code: String,
    display_name: String,
    #[serde(default)]
    avatar_sha1: Option<String>,
}

#[derive(Deserialize)]
struct MessageRow {
    id: i64,
    sender: String,
    recipient: String,
    body: String,
    created_at: String,
    read_at: Option<String>,
}

impl From<MessageRow> for ChatMessage {
    fn from(m: MessageRow) -> Self {
        Self {
            id: m.id,
            sender: m.sender,
            recipient: m.recipient,
            body: m.body,
            created_at: m.created_at,
            read_at: m.read_at,
        }
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Turns a non-2xx response into an error, recognising the server's codes.
fn server_error(status: u16, body: &[u8]) -> CoreError {
    let v: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
    let message = v["message"]
        .as_str()
        .or_else(|| v["msg"].as_str())
        .or_else(|| v["error_description"].as_str())
        .or_else(|| v["error"].as_str())
        .unwrap_or("")
        .to_owned();
    if let Some(code) = FRIEND_ERRORS
        .iter()
        .chain(names::NAME_ERRORS)
        .chain(textures::TEXTURE_ERRORS)
        .chain(library::LIBRARY_ERRORS)
        .chain(account::AUTH_ERRORS)
        .chain(admin::ADMIN_ERRORS)
        .find(|c| **c == message)
    {
        return CoreError::Friends(code);
    }
    if let Some(code) = account::auth_error(v["error_code"].as_str().unwrap_or(""), &message) {
        return CoreError::Friends(code);
    }
    let pg_code = v["code"].as_str().unwrap_or("");
    let error_code = v["error_code"].as_str().unwrap_or("");
    if error_code == "anonymous_provider_disabled" || message.contains("Anonymous sign-ins") {
        return CoreError::Friends("friends.unavailable");
    }
    // Row-level security refused the insert (e.g. message to a non-friend).
    if pg_code == "42501" {
        return CoreError::Friends("friends.notFriends");
    }
    // Check constraint (e.g. empty or too long message).
    if pg_code == "23514" {
        return CoreError::Friends("friends.messageInvalid");
    }
    if status == 429 {
        return CoreError::Friends("friends.rateLimited");
    }
    CoreError::FriendsServer {
        status,
        reason: if message.is_empty() {
            String::from_utf8_lossy(body).chars().take(200).collect()
        } else {
            message
        },
    }
}

/// Client for the friends service. Cheap to share behind an `Arc`.
pub struct FriendsClient {
    ctx: Ctx,
    session: Mutex<Option<Session>>,
    online: std::sync::atomic::AtomicBool,
    /// Live tests may still create anonymous identities (K73); the
    /// launcher never does.
    allow_anonymous: std::sync::atomic::AtomicBool,
}

impl FriendsClient {
    pub fn new(ctx: Ctx) -> Self {
        let session = std::fs::read(ctx.paths.friends_file())
            .ok()
            .and_then(|b| serde_json::from_slice::<Session>(&b).ok());
        Self {
            ctx,
            session: Mutex::new(session),
            online: std::sync::atomic::AtomicBool::new(false),
            allow_anonymous: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(crate) fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.ctx.endpoints.friends.trim_end_matches('/'))
    }

    /// Friends are on (the user agreed; an identity exists).
    pub async fn is_enabled(&self) -> bool {
        self.session
            .lock()
            .await
            .as_ref()
            .is_some_and(|s| s.friends_enabled)
    }

    /// An anonymous identity exists (friends on or off).
    pub async fn has_identity(&self) -> bool {
        self.session.lock().await.is_some()
    }

    /// Lets [`Self::ensure_identity`] create an anonymous identity. Only for
    /// tests: the server refuses them once anonymous sign-ins are off, and
    /// the launcher signs in with an email account instead (K73).
    pub fn allow_anonymous(&self) {
        self.allow_anonymous
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// The signed-in identity; without one `auth.required` (the launcher
    /// shows the sign-in screen).
    pub(crate) async fn ensure_identity(&self) -> Result<()> {
        if self.has_identity().await {
            return Ok(());
        }
        if !self
            .allow_anonymous
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(CoreError::Friends("auth.required"));
        }
        let s = self.auth_call("/auth/v1/signup", json!({})).await?;
        self.save(&s)?;
        *self.session.lock().await = Some(s);
        Ok(())
    }

    async fn set_friends_enabled(&self, on: bool) -> Result<()> {
        let mut guard = self.session.lock().await;
        if let Some(s) = guard.as_mut() {
            s.friends_enabled = on;
            write_json_atomic(&self.ctx.paths.friends_file(), &*s)?;
        }
        Ok(())
    }

    pub(crate) async fn user_id(&self) -> Result<String> {
        self.session
            .lock()
            .await
            .as_ref()
            .map(|s| s.user_id.clone())
            .ok_or(CoreError::Friends("friends.disabled"))
    }

    fn save(&self, s: &Session) -> Result<()> {
        write_json_atomic(&self.ctx.paths.friends_file(), s)
    }

    async fn auth_call(&self, path: &str, body: Value) -> Result<Session> {
        let (status, bytes) = self
            .ctx
            .http
            .request_json(
                Method::POST,
                &self.url(path),
                Some(&body),
                &[("apikey", ANON_KEY)],
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(server_error(status, &bytes));
        }
        let v: Value = serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
            path: path.into(),
            source,
        })?;
        let field = |k: &str| v[k].as_str().map(str::to_owned);
        match (
            v["user"]["id"].as_str(),
            field("access_token"),
            field("refresh_token"),
        ) {
            (Some(id), Some(access), Some(refresh)) => Ok(Session {
                email: v["user"]["email"]
                    .as_str()
                    .filter(|e| !e.is_empty())
                    .map(str::to_owned),
                anonymous: v["user"]["is_anonymous"].as_bool().unwrap_or(false),
                account: account::AccountCache::default(),
                user_id: id.to_owned(),
                access_token: access,
                refresh_token: refresh,
                expires_at: v["expires_at"]
                    .as_u64()
                    .unwrap_or_else(|| now_secs() + v["expires_in"].as_u64().unwrap_or(3600)),
                friends_enabled: false,
            }),
            _ => Err(CoreError::FriendsServer {
                status,
                reason: "unexpected auth response".into(),
            }),
        }
    }

    /// A valid access token, refreshed shortly before it expires.
    async fn token(&self) -> Result<String> {
        let mut guard = self.session.lock().await;
        let s = guard
            .as_ref()
            .ok_or(CoreError::Friends("friends.disabled"))?;
        if s.expires_at > now_secs() + 60 {
            return Ok(s.access_token.clone());
        }
        let refreshed = self
            .auth_call(
                "/auth/v1/token?grant_type=refresh_token",
                json!({ "refresh_token": s.refresh_token }),
            )
            .await?;
        let refreshed = Session {
            friends_enabled: s.friends_enabled,
            account: s.account.clone(),
            email: refreshed.email.or_else(|| s.email.clone()),
            ..refreshed
        };
        self.save(&refreshed)?;
        let token = refreshed.access_token.clone();
        *guard = Some(refreshed);
        Ok(token)
    }

    /// Authenticated request; `(status, body)` with server errors mapped.
    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        extra: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        let token = self.token().await?;
        let bearer = format!("Bearer {token}");
        let mut headers = vec![("apikey", ANON_KEY), ("Authorization", bearer.as_str())];
        headers.extend_from_slice(extra);
        let (status, bytes) = self
            .ctx
            .http
            .request_json(method, &self.url(path), body, &headers)
            .await?;
        if (200..300).contains(&status) {
            Ok(bytes)
        } else {
            Err(server_error(status, &bytes))
        }
    }

    pub(crate) async fn send_raw(
        &self,
        method: Method,
        path: &str,
        body: Vec<u8>,
        content_type: &str,
        extra: &[(&str, &str)],
    ) -> Result<Vec<u8>> {
        let token = self.token().await?;
        let bearer = format!("Bearer {token}");
        let mut headers = vec![("apikey", ANON_KEY), ("Authorization", bearer.as_str())];
        headers.extend_from_slice(extra);
        let (status, bytes) = self
            .ctx
            .http
            .request_bytes(
                method,
                &self.url(path),
                body,
                content_type,
                &headers,
                std::time::Duration::from_secs(600),
            )
            .await?;
        if (200..300).contains(&status) {
            Ok(bytes)
        } else {
            Err(server_error(status, &bytes))
        }
    }

    pub(crate) async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Value>,
        extra: &[(&str, &str)],
    ) -> Result<T> {
        let bytes = self.send(method, path, body, extra).await?;
        serde_json::from_slice(&bytes).map_err(|source| CoreError::Json {
            path: path.into(),
            source,
        })
    }

    async fn rpc<T: DeserializeOwned>(&self, name: &str, args: Value) -> Result<T> {
        self.json(
            Method::POST,
            &format!("/rest/v1/rpc/{name}"),
            Some(&args),
            &[],
        )
        .await
    }

    async fn rpc_void(&self, name: &str, args: Value) -> Result<()> {
        self.send(
            Method::POST,
            &format!("/rest/v1/rpc/{name}"),
            Some(&args),
            &[],
        )
        .await
        .map(drop)
    }

    /// Creates the anonymous identity (once) and the profile shown to
    /// friends under `display_name` with the photo `avatar` (PNG).
    pub async fn enable(&self, display_name: &str, avatar: Option<&[u8]>) -> Result<Profile> {
        self.ensure_identity().await?;
        let profile = self.sync_profile(display_name, avatar).await?;
        self.set_friends_enabled(true).await?;
        // Online right away instead of after the first timer tick.
        if let Err(e) = self.heartbeat().await {
            tracing::debug!(error = %e.detail(), "first heartbeat failed");
        }
        Ok(profile)
    }

    /// Creates or renames the caller's profile and updates its photo. A
    /// failed photo upload does not fail the profile.
    pub async fn sync_profile(&self, display_name: &str, avatar: Option<&[u8]>) -> Result<Profile> {
        let p: ProfileRow = self
            .rpc("ensure_profile", json!({ "name": display_name }))
            .await?;
        if let Err(e) = self.sync_avatar(p.avatar_sha1.as_deref(), avatar).await {
            tracing::warn!(error = %e.detail(), "could not update the profile photo");
        }
        Ok(Profile {
            id: p.id,
            friend_code: p.friend_code,
            display_name: p.display_name,
        })
    }

    pub async fn status(&self, display_name: &str, avatar: Option<&[u8]>) -> Result<FriendsStatus> {
        if !self.is_enabled().await {
            return Ok(FriendsStatus {
                enabled: false,
                profile: None,
                online: false,
            });
        }
        let profile = self.sync_profile(display_name, avatar).await?;
        Ok(FriendsStatus {
            enabled: true,
            profile: Some(profile),
            online: self.is_online(),
        })
    }

    /// Removes uploaded files and photos (Storage API; SQL cannot).
    async fn delete_files(&self) -> Result<()> {
        for deleted in [
            self.delete_all_uploads().await,
            self.delete_all_avatars().await,
        ] {
            match deleted {
                // An identity the server already forgot has nothing left.
                Ok(()) | Err(CoreError::FriendsServer { status: 401, .. }) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// "Turn off friends and delete my data": profile, friendships,
    /// messages, shared lists and files go; the identity and the account
    /// names it reserved stay (K67).
    pub async fn disable_and_delete(&self) -> Result<()> {
        if self.is_enabled().await {
            self.delete_files().await?;
            match self.rpc_void("delete_friend_data", json!({})).await {
                Ok(()) | Err(CoreError::FriendsServer { status: 401, .. }) => {}
                Err(e) => return Err(e),
            }
        }
        self.online
            .store(false, std::sync::atomic::Ordering::Relaxed);
        self.set_friends_enabled(false).await
    }

    /// Deletes the whole identity on the server (including reserved names)
    /// and forgets it locally.
    pub async fn delete_identity(&self) -> Result<()> {
        if self.has_identity().await {
            self.delete_files().await?;
            // Shared skins/capes and library mods belong to the identity,
            // not to friends.
            for deleted in [
                self.delete_all_textures().await,
                self.delete_all_library_files().await,
            ] {
                match deleted {
                    Ok(()) | Err(CoreError::FriendsServer { status: 401, .. }) => {}
                    Err(e) => return Err(e),
                }
            }
            match self.rpc_void("delete_me", json!({})).await {
                Ok(()) | Err(CoreError::FriendsServer { status: 401, .. }) => {}
                Err(e) => return Err(e),
            }
        }
        *self.session.lock().await = None;
        self.online
            .store(false, std::sync::atomic::Ordering::Relaxed);
        match std::fs::remove_file(self.ctx.paths.friends_file()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CoreError::io(self.ctx.paths.friends_file(), e)),
        }
    }

    pub async fn friends(&self) -> Result<Vec<Friend>> {
        let rows: Vec<FriendRow> = self.rpc("my_friends", json!({})).await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let avatar = match (&r.avatar_sha1, r.status) {
                (Some(sha), FriendStatus::Accepted) => self.friend_avatar(&r.id, sha).await,
                _ => None,
            };
            out.push(Friend {
                id: r.id,
                friend_code: r.friend_code,
                display_name: r.display_name,
                status: r.status,
                incoming: r.incoming,
                request_id: r.request_id,
                unread: r.unread,
                avatar,
                online: r.online && r.status == FriendStatus::Accepted,
            });
        }
        Ok(out)
    }

    /// Returns `"pending"` or `"accepted"` (when they had asked me already).
    pub async fn send_request(&self, code: &str) -> Result<String> {
        let code = code.trim().to_ascii_uppercase();
        if code.is_empty() || code.len() > 32 {
            return Err(CoreError::Friends("friends.codeNotFound"));
        }
        self.rpc("send_request", json!({ "code": code })).await
    }

    pub async fn respond(&self, request_id: i64, accept: bool) -> Result<()> {
        self.rpc_void(
            "respond_request",
            json!({ "request_id": request_id, "accept": accept }),
        )
        .await
    }

    pub async fn remove(&self, friend: &str) -> Result<()> {
        self.rpc_void("remove_friend", json!({ "friend": friend }))
            .await
    }

    pub async fn block(&self, friend: &str) -> Result<()> {
        self.rpc_void("block_user", json!({ "target": friend }))
            .await
    }

    /// Conversation with `friend`: messages after `after` (oldest first), or
    /// the latest 100 when `after` is `None`.
    pub async fn messages(&self, friend: &str, after: Option<i64>) -> Result<Vec<ChatMessage>> {
        let me = self.user_id().await?;
        check_uuid(friend)?;
        let pair = format!(
            "(and(sender.eq.{me},recipient.eq.{friend}),and(sender.eq.{friend},recipient.eq.{me}))"
        );
        let mut url = reqwest::Url::parse("http://x/rest/v1/messages").expect("static url");
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("select", "id,sender,recipient,body,created_at,read_at");
            q.append_pair("or", &pair);
            match after {
                Some(id) => {
                    q.append_pair("id", &format!("gt.{id}"));
                    q.append_pair("order", "id.asc");
                    q.append_pair("limit", "200");
                }
                None => {
                    q.append_pair("order", "id.desc");
                    q.append_pair("limit", "100");
                }
            }
        }
        let path = format!("{}?{}", url.path(), url.query().unwrap_or(""));
        let mut rows: Vec<MessageRow> = self.json(Method::GET, &path, None, &[]).await?;
        if after.is_none() {
            rows.reverse();
        }
        Ok(rows.into_iter().map(ChatMessage::from).collect())
    }

    pub async fn send_message(&self, friend: &str, body: &str) -> Result<ChatMessage> {
        let body = body.trim();
        if body.is_empty() || body.chars().count() > MAX_MESSAGE_CHARS {
            return Err(CoreError::Friends("friends.messageInvalid"));
        }
        check_uuid(friend)?;
        let me = self.user_id().await?;
        let mut rows: Vec<MessageRow> = self
            .json(
                Method::POST,
                "/rest/v1/messages?select=id,sender,recipient,body,created_at,read_at",
                Some(&json!({ "sender": me, "recipient": friend, "body": body })),
                &[("Prefer", "return=representation")],
            )
            .await?;
        rows.pop()
            .map(ChatMessage::from)
            .ok_or_else(|| CoreError::FriendsServer {
                status: 201,
                reason: "empty insert response".into(),
            })
    }

    pub async fn mark_read(&self, friend: &str) -> Result<()> {
        check_uuid(friend)?;
        self.rpc_void("mark_read", json!({ "friend": friend }))
            .await
    }
}

/// Ids end up in PostgREST filters; only accept real UUIDs.
pub(crate) fn check_uuid(id: &str) -> Result<()> {
    let ok = id.len() == 36
        && id.chars().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        });
    if ok {
        Ok(())
    } else {
        Err(CoreError::Friends("friends.notFriends"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::NullSink;
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;

    pub const ME: &str = "11111111-1111-1111-1111-111111111111";
    pub const FRIEND: &str = "22222222-2222-2222-2222-222222222222";

    pub fn ctx(tmp: &tempfile::TempDir, server: &MockServer) -> Ctx {
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        ctx.endpoints.friends = server.uri();
        ctx.endpoints.modrinth = server.uri();
        ctx
    }

    pub fn auth_body(access: &str, expires_in: u64) -> Value {
        json!({
            "access_token": access,
            "refresh_token": "refresh-1",
            "expires_in": expires_in,
            "user": { "id": ME }
        })
    }

    /// A client that already has a session (no signup round trip).
    pub async fn signed_in(tmp: &tempfile::TempDir, server: &MockServer) -> FriendsClient {
        let c = FriendsClient::new(ctx(tmp, server));
        *c.session.lock().await = Some(Session {
            user_id: ME.into(),
            access_token: "tok".into(),
            refresh_token: "refresh-1".into(),
            expires_at: now_secs() + 3600,
            friends_enabled: true,
            email: Some("me@example.com".into()),
            anonymous: false,
            account: account::AccountCache::default(),
        });
        c
    }

    #[tokio::test]
    async fn enables_with_anonymous_signup_and_persists_session() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/signup"))
            .and(header("apikey", ANON_KEY))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_body("tok", 3600)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/ensure_profile"))
            .and(header("authorization", "Bearer tok"))
            .and(body_partial_json(json!({ "name": "Mehbur" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": ME, "friend_code": "MEHBUR-7K3Q", "display_name": "Mehbur",
                "created_at": "2026-10-05T10:00:00Z"
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        c.allow_anonymous();
        assert!(!c.status("Mehbur", None).await.unwrap().enabled);
        let p = c.enable("Mehbur", None).await.unwrap();
        assert_eq!(p.friend_code, "MEHBUR-7K3Q");
        // A new client (next launcher start) reuses the stored identity.
        let again = FriendsClient::new(c.ctx().clone());
        assert_eq!(again.user_id().await.unwrap(), ME);
    }

    #[tokio::test]
    async fn refreshes_an_expiring_token() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "refresh_token"))
            .and(body_partial_json(json!({ "refresh_token": "refresh-1" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(auth_body("fresh", 3600)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/my_friends"))
            .and(header("authorization", "Bearer fresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": FRIEND, "friend_code": "MEHBUR-AAAA", "display_name": "Ali",
                "status": "pending", "incoming": true, "request_id": 7, "unread": 0,
                "blocked_by_me": false
            }])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.session.lock().await.as_mut().unwrap().expires_at = now_secs();
        let f = c.friends().await.unwrap();
        assert_eq!(f[0].status, FriendStatus::Pending);
        assert!(f[0].incoming);
        // The refreshed session is what gets stored.
        let stored = std::fs::read_to_string(c.ctx().paths.friends_file()).unwrap();
        assert!(stored.contains("fresh"));
    }

    #[tokio::test]
    async fn maps_server_refusals_to_codes() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/send_request"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "code": "P0001", "message": "friends.codeNotFound"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/messages"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "code": "42501", "message": "new row violates row-level security policy"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/signup"))
            .respond_with(ResponseTemplate::new(422).set_body_json(json!({
                "code": 422, "error_code": "anonymous_provider_disabled",
                "msg": "Anonymous sign-ins are disabled"
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let e = c.send_request(" mehbur-zzzz ").await.unwrap_err();
        assert_eq!(e.code(), "friends.codeNotFound");
        let e = c.send_message(FRIEND, "hi").await.unwrap_err();
        assert_eq!(e.code(), "friends.notFriends");
        let e = c.send_message(FRIEND, "  ").await.unwrap_err();
        assert_eq!(e.code(), "friends.messageInvalid");
        let e = c.messages("x' or 1=1", None).await.unwrap_err();
        assert_eq!(e.code(), "friends.notFriends");

        let tmp2 = tempfile::tempdir().unwrap();
        let fresh = FriendsClient::new(ctx(&tmp2, &server));
        fresh.allow_anonymous();
        assert_eq!(
            fresh.enable("A", None).await.unwrap_err().code(),
            "friends.unavailable"
        );
        assert_eq!(
            fresh.friends().await.unwrap_err().code(),
            "friends.disabled"
        );
    }

    #[tokio::test]
    async fn loads_conversations_incrementally() {
        let server = MockServer::builder().start().await;
        let msg = |id: i64, from: &str, to: &str| {
            json!({ "id": id, "sender": from, "recipient": to, "body": format!("m{id}"),
                    "created_at": "2026-10-05T10:00:00Z", "read_at": null })
        };
        Mock::given(method("GET"))
            .and(path("/rest/v1/messages"))
            .and(query_param("order", "id.desc"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!([msg(2, FRIEND, ME), msg(1, ME, FRIEND)])),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/v1/messages"))
            .and(query_param("id", "gt.2"))
            .and(query_param("order", "id.asc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([msg(3, FRIEND, ME)])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        let first = c.messages(FRIEND, None).await.unwrap();
        assert_eq!(first.iter().map(|m| m.id).collect::<Vec<_>>(), [1, 2]);
        let next = c.messages(FRIEND, Some(2)).await.unwrap();
        assert_eq!(next[0].body, "m3");
    }

    #[tokio::test]
    async fn delete_forgets_the_identity() {
        let server = MockServer::builder().start().await;
        // Uploads go first, through the Storage API.
        Mock::given(method("POST"))
            .and(path("/storage/v1/object/list/avatars"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/storage/v1/object/list/mods"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{ "name": "a.jar" }])))
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/storage/v1/object/mods"))
            .and(wiremock::matchers::body_json(
                json!({ "prefixes": [format!("{ME}/a.jar")] }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(2) // friends off, then the whole identity
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/delete_friend_data"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        // Shared skins/capes go only with the whole identity.
        Mock::given(method("POST"))
            .and(path("/storage/v1/object/list/textures"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{ "name": "s.png" }])))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path("/storage/v1/object/textures"))
            .and(wiremock::matchers::body_json(
                json!({ "prefixes": [format!("{ME}/s.png")] }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .expect(1)
            .mount(&server)
            .await;
        // No library bucket on this server: deleting still works.
        Mock::given(method("POST"))
            .and(path("/storage/v1/object/list/library"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({ "error": "Bucket not found" })),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/delete_me"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.save(&c.session.lock().await.clone().unwrap()).unwrap();

        // Friends off: data goes, the identity (and its names) stays.
        c.disable_and_delete().await.unwrap();
        assert!(!c.is_enabled().await);
        assert!(c.has_identity().await);
        let reloaded = FriendsClient::new(c.ctx().clone());
        assert!(reloaded.has_identity().await && !reloaded.is_enabled().await);

        // Whole identity.
        c.delete_identity().await.unwrap();
        assert!(!c.has_identity().await);
        assert!(!c.ctx().paths.friends_file().exists());
    }

    #[test]
    fn sessions_saved_before_names_count_as_friends_on() {
        let old = json!({ "userId": ME, "accessToken": "a", "refreshToken": "r", "expiresAt": 1 });
        let s: Session = serde_json::from_value(old).unwrap();
        assert!(s.friends_enabled);
    }
}
