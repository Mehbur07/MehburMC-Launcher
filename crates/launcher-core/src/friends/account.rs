//! MehburMC accounts (ARCHITECTURE K73): Supabase Auth email + password.
//! Every email step is confirmed with the 6-digit code from the email, so
//! nothing has to leave the launcher. An anonymous identity of an older
//! launcher is upgraded in place (email, then code, then password), keeping
//! its friends, names and shares. The password is only ever sent to
//! Supabase; the launcher keeps the session tokens, never the password.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use ts_rs::TS;

use super::{ANON_KEY, FriendsClient, Session, server_error};
use crate::error::{CoreError, Result};

/// Codes of this module; each has an `errors.auth.*` translation.
pub const AUTH_ERRORS: &[&str] = &[
    "auth.required",
    "auth.banned",
    "auth.invalidCredentials",
    "auth.emailTaken",
    "auth.codeInvalid",
    "auth.weakPassword",
    "auth.emailInvalid",
    "auth.rateLimited",
    "auth.emailNotConfirmed",
    "auth.signupDisabled",
    "auth.samePassword",
    "auth.sessionExpired",
];

const MIN_PASSWORD: usize = 8;
/// bcrypt only uses the first 72 bytes.
const MAX_PASSWORD_BYTES: usize = 72;

/// Supabase Auth `error_code`s → our codes.
pub(crate) fn auth_error(error_code: &str, message: &str) -> Option<&'static str> {
    Some(match error_code {
        "user_banned" => "auth.banned",
        "invalid_credentials" => "auth.invalidCredentials",
        "email_exists" | "user_already_exists" | "identity_already_exists" => "auth.emailTaken",
        "otp_expired" | "otp_disabled" | "bad_code_verifier" | "flow_state_expired" => {
            "auth.codeInvalid"
        }
        "weak_password" => "auth.weakPassword",
        "email_address_invalid" | "email_address_not_authorized" => "auth.emailInvalid",
        "over_email_send_rate_limit" | "over_request_rate_limit" => "auth.rateLimited",
        "email_not_confirmed" => "auth.emailNotConfirmed",
        "signup_disabled" | "email_provider_disabled" => "auth.signupDisabled",
        "same_password" => "auth.samePassword",
        "refresh_token_not_found"
        | "refresh_token_already_used"
        | "session_not_found"
        | "user_not_found" => "auth.sessionExpired",
        _ if message.contains("Invalid Refresh Token") => "auth.sessionExpired",
        _ if message.contains("Token has expired or is invalid") => "auth.codeInvalid",
        _ => return None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BanInfo {
    /// RFC 3339; `None` = permanent (or unknown when sign-in was refused).
    pub until: Option<String>,
    pub reason: String,
}

/// Last rank/ban the server reported, kept in `friends.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountCache {
    #[serde(default)]
    rank: u8,
    #[serde(default)]
    ban: Option<BanInfo>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthStatus {
    /// Signed in with an email account (the launcher is usable).
    pub signed_in: bool,
    pub email: Option<String>,
    /// An anonymous identity of an older launcher exists: signing up
    /// upgrades it and keeps its friends, names and shares.
    pub anonymous_identity: bool,
    /// Admin rank: 0 none, 1 founder, 2 admin.
    pub rank: u8,
    pub ban: Option<BanInfo>,
    /// The server could not be reached; rank/ban are the last known ones.
    pub offline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CodePurpose {
    /// New account.
    Signup,
    /// Email added to an anonymous identity.
    Upgrade,
    /// Forgotten password.
    Recovery,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
#[ts(export)]
pub enum SignUpResult {
    SignedIn,
    /// A code was emailed; confirm with `verify_code(purpose)`.
    CodeSent {
        purpose: CodePurpose,
    },
}

#[derive(Deserialize)]
struct MyAccountRow {
    rank: i32,
    banned: bool,
    ban_until: Option<String>,
    ban_reason: Option<String>,
}

/// Trimmed, lower-case email with a basic shape check.
pub fn normalize_email(email: &str) -> Result<String> {
    let e = email.trim().to_lowercase();
    let ok = e.len() <= 254
        && !e.chars().any(|c| c.is_whitespace() || c.is_control())
        && e.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains('@')
        });
    if ok {
        Ok(e)
    } else {
        Err(CoreError::Friends("auth.emailInvalid"))
    }
}

pub fn check_password(password: &str) -> Result<()> {
    if password.chars().count() < MIN_PASSWORD || password.len() > MAX_PASSWORD_BYTES {
        return Err(CoreError::Friends("auth.weakPassword"));
    }
    Ok(())
}

/// The emailed code: digits only (Supabase sends 6–10).
fn clean_code(code: &str) -> Result<String> {
    let c: String = code.chars().filter(|c| !c.is_whitespace()).collect();
    if (6..=10).contains(&c.len()) && c.bytes().all(|b| b.is_ascii_digit()) {
        Ok(c)
    } else {
        Err(CoreError::Friends("auth.codeInvalid"))
    }
}

impl FriendsClient {
    /// POST to Supabase Auth without a session (apikey only).
    async fn public_post(&self, path: &str, body: Value) -> Result<Value> {
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
        Ok(serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn install_session(&self, mut s: Session, friends_enabled: bool) -> Result<()> {
        s.friends_enabled = friends_enabled;
        self.save(&s)?;
        *self.session.lock().await = Some(s);
        Ok(())
    }

    async fn snapshot(&self) -> Option<Session> {
        self.session.lock().await.clone()
    }

    /// Creates an account. With an anonymous identity, adds the email to it
    /// (the password follows after the code); otherwise signs up.
    pub async fn sign_up(&self, email: &str, password: &str) -> Result<SignUpResult> {
        let email = normalize_email(email)?;
        check_password(password)?;
        match self.snapshot().await {
            Some(s) if s.anonymous => {
                self.send(
                    Method::PUT,
                    "/auth/v1/user",
                    Some(&json!({ "email": email })),
                    &[],
                )
                .await?;
                Ok(SignUpResult::CodeSent {
                    purpose: CodePurpose::Upgrade,
                })
            }
            _ => {
                let v = self
                    .public_post(
                        "/auth/v1/signup",
                        json!({ "email": email, "password": password }),
                    )
                    .await?;
                // "Confirm email" off: the session comes right away.
                if v["access_token"].is_string() {
                    self.install_session(session_from(&v)?, false).await?;
                    return Ok(SignUpResult::SignedIn);
                }
                Ok(SignUpResult::CodeSent {
                    purpose: CodePurpose::Signup,
                })
            }
        }
    }

    /// Confirms the emailed code. `password` is set for upgrades and
    /// password resets (ignored for a plain sign-up, which already has it).
    pub async fn verify_code(
        &self,
        email: &str,
        code: &str,
        purpose: CodePurpose,
        password: &str,
    ) -> Result<()> {
        let email = normalize_email(email)?;
        let code = clean_code(code)?;
        if purpose != CodePurpose::Signup {
            check_password(password)?;
        }
        let kind = match purpose {
            CodePurpose::Signup => "signup",
            CodePurpose::Upgrade => "email_change",
            CodePurpose::Recovery => "recovery",
        };
        let before = self.snapshot().await;
        let friends_on = before.as_ref().is_some_and(|s| s.friends_enabled);
        let body = json!({ "type": kind, "email": email, "token": code });
        match purpose {
            CodePurpose::Upgrade => {
                // The verify answer may or may not carry a new session; either
                // way the next refresh has the email and is_anonymous = false.
                let v = self.public_post("/auth/v1/verify", body).await?;
                if v["access_token"].is_string() {
                    let s = session_from(&v)?;
                    self.install_session(s, friends_on).await?;
                } else {
                    self.expire_token().await;
                }
                self.set_password(password).await?;
                // Pick up the upgraded claims (email, not anonymous).
                self.expire_token().await;
                self.token().await?;
            }
            CodePurpose::Signup | CodePurpose::Recovery => {
                let s = self.auth_call("/auth/v1/verify", body).await?;
                // A different account than the one stored here (recovery on
                // a shared PC): start without friends consent.
                let same = before.as_ref().is_some_and(|b| b.user_id == s.user_id);
                self.install_session(s, same && friends_on).await?;
                if purpose == CodePurpose::Recovery {
                    self.set_password(password).await?;
                }
            }
        }
        Ok(())
    }

    async fn set_password(&self, password: &str) -> Result<()> {
        match self
            .send(
                Method::PUT,
                "/auth/v1/user",
                Some(&json!({ "password": password })),
                &[],
            )
            .await
        {
            // Resetting to the same password is fine.
            Err(CoreError::Friends("auth.samePassword")) => Ok(()),
            r => r.map(drop),
        }
    }

    /// Forces the next request to refresh the session.
    async fn expire_token(&self) {
        if let Some(s) = self.session.lock().await.as_mut() {
            s.expires_at = 0;
        }
    }

    /// Sends the code again.
    pub async fn resend_code(&self, email: &str, purpose: CodePurpose) -> Result<()> {
        let email = normalize_email(email)?;
        match purpose {
            CodePurpose::Recovery => self.request_password_reset(&email).await,
            CodePurpose::Signup => self
                .public_post(
                    "/auth/v1/resend",
                    json!({ "type": "signup", "email": email }),
                )
                .await
                .map(drop),
            CodePurpose::Upgrade => self
                .send(
                    Method::PUT,
                    "/auth/v1/user",
                    Some(&json!({ "email": email })),
                    &[],
                )
                .await
                .map(drop),
        }
    }

    /// Emails a password reset code.
    pub async fn request_password_reset(&self, email: &str) -> Result<()> {
        let email = normalize_email(email)?;
        self.public_post("/auth/v1/recover", json!({ "email": email }))
            .await
            .map(drop)
    }

    /// Signs in with email + password. An anonymous identity stored here is
    /// deleted first (its names are released; the UI warns about it).
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<()> {
        let email = normalize_email(email)?;
        if password.is_empty() {
            return Err(CoreError::Friends("auth.invalidCredentials"));
        }
        let s = self
            .auth_call(
                "/auth/v1/token?grant_type=password",
                json!({ "email": email, "password": password }),
            )
            .await?;
        if let Some(old) = self.snapshot().await
            && old.anonymous
            && old.user_id != s.user_id
            && let Err(e) = self.delete_identity().await
        {
            tracing::warn!(error = %e.detail(), "could not delete the old anonymous identity");
        }
        self.install_session(s, false).await
    }

    /// Signs out here (other devices stay signed in).
    pub async fn sign_out(&self) -> Result<()> {
        if self.has_identity().await {
            if let Err(e) = self.go_offline().await {
                tracing::debug!(error = %e.detail(), "go_offline before sign-out failed");
            }
            if let Err(e) = self
                .send(Method::POST, "/auth/v1/logout?scope=local", None, &[])
                .await
            {
                tracing::debug!(error = %e.detail(), "logout request failed");
            }
        }
        self.forget_session().await
    }

    async fn forget_session(&self) -> Result<()> {
        *self.session.lock().await = None;
        self.online
            .store(false, std::sync::atomic::Ordering::Relaxed);
        match std::fs::remove_file(self.ctx.paths.friends_file()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CoreError::io(self.ctx.paths.friends_file(), e)),
        }
    }

    /// Account state for the sign-in gate. With `check` the server is asked
    /// for rank and ban (offline → the last known ones).
    pub async fn auth_status(&self, check: bool) -> AuthStatus {
        let signed_out = |anonymous_identity| AuthStatus {
            signed_in: false,
            email: None,
            anonymous_identity,
            rank: 0,
            ban: None,
            offline: false,
        };
        let Some(s) = self.snapshot().await else {
            return signed_out(false);
        };
        if s.anonymous {
            return signed_out(true);
        }
        let mut status = AuthStatus {
            signed_in: true,
            email: s.email.clone(),
            anonymous_identity: false,
            rank: s.account.rank,
            ban: s.account.ban.clone(),
            offline: !check,
        };
        if !check {
            return status;
        }
        let cache = match self.rpc::<Vec<MyAccountRow>>("my_account", json!({})).await {
            Ok(rows) => {
                let row = rows.into_iter().next();
                AccountCache {
                    rank: row.as_ref().map(|r| r.rank.clamp(0, 2) as u8).unwrap_or(0),
                    ban: row.filter(|r| r.banned).map(|r| BanInfo {
                        until: r.ban_until,
                        reason: r.ban_reason.unwrap_or_default(),
                    }),
                }
            }
            Err(CoreError::Friends("auth.banned")) => AccountCache {
                rank: 0,
                ban: Some(s.account.ban.clone().unwrap_or_default()),
            },
            Err(CoreError::Friends("auth.sessionExpired")) => {
                if let Err(e) = self.forget_session().await {
                    tracing::warn!(error = %e.detail(), "could not forget an expired session");
                }
                return signed_out(false);
            }
            Err(e) => {
                tracing::debug!(error = %e.detail(), "account check failed, using the cache");
                status.offline = true;
                return status;
            }
        };
        status.rank = cache.rank;
        status.ban = cache.ban.clone();
        status.offline = false;
        let mut guard = self.session.lock().await;
        if let Some(cur) = guard.as_mut()
            && cur.user_id == s.user_id
        {
            cur.account = cache;
            if let Err(e) = self.save(cur) {
                tracing::debug!(error = %e.detail(), "could not cache the account state");
            }
        }
        status
    }

    /// Signed in with an email account and not known to be banned.
    pub async fn can_play(&self) -> bool {
        self.snapshot()
            .await
            .is_some_and(|s| !s.anonymous && s.account.ban.is_none())
    }
}

fn session_from(v: &Value) -> Result<Session> {
    let field = |k: &str| v[k].as_str().map(str::to_owned);
    match (
        v["user"]["id"].as_str(),
        field("access_token"),
        field("refresh_token"),
    ) {
        (Some(id), Some(access), Some(refresh)) => Ok(Session {
            user_id: id.to_owned(),
            access_token: access,
            refresh_token: refresh,
            expires_at: v["expires_at"]
                .as_u64()
                .unwrap_or_else(|| super::now_secs() + v["expires_in"].as_u64().unwrap_or(3600)),
            friends_enabled: false,
            email: v["user"]["email"].as_str().map(str::to_owned),
            anonymous: v["user"]["is_anonymous"].as_bool().unwrap_or(false),
            account: AccountCache::default(),
        }),
        _ => Err(CoreError::FriendsServer {
            status: 200,
            reason: "unexpected auth response".into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::{ME, ctx, signed_in};
    use super::*;

    fn session_body(access: &str, anonymous: bool) -> Value {
        json!({
            "access_token": access,
            "refresh_token": format!("r-{access}"),
            "expires_in": 3600,
            "user": { "id": ME, "email": if anonymous { "" } else { "a@b.co" }, "is_anonymous": anonymous }
        })
    }

    async fn anonymous(tmp: &tempfile::TempDir, server: &MockServer) -> FriendsClient {
        let c = signed_in(tmp, server).await;
        {
            let mut g = c.session.lock().await;
            let s = g.as_mut().unwrap();
            s.anonymous = true;
            s.email = None;
        }
        c
    }

    #[test]
    fn input_checks() {
        assert_eq!(normalize_email(" A@B.Co ").unwrap(), "a@b.co");
        for bad in ["", "ab", "a@b", "@b.co", "a@.co", "a b@c.co", "a@b.co."] {
            assert!(normalize_email(bad).is_err(), "{bad}");
        }
        assert!(check_password("1234567").is_err());
        check_password("12345678").unwrap();
        assert!(check_password(&"x".repeat(73)).is_err());
        assert_eq!(clean_code(" 123 456 ").unwrap(), "123456");
        assert!(clean_code("12a456").is_err() && clean_code("123").is_err());
    }

    #[test]
    fn maps_auth_errors() {
        let e = |code: &str, msg: &str| {
            server_error(
                400,
                &serde_json::to_vec(&json!({ "error_code": code, "msg": msg })).unwrap(),
            )
            .code()
            .to_owned()
        };
        assert_eq!(e("invalid_credentials", ""), "auth.invalidCredentials");
        assert_eq!(e("user_banned", ""), "auth.banned");
        assert_eq!(e("email_exists", ""), "auth.emailTaken");
        assert_eq!(e("otp_expired", ""), "auth.codeInvalid");
        assert_eq!(
            e("", "Invalid Refresh Token: Already Used"),
            "auth.sessionExpired"
        );
        assert_eq!(e("over_email_send_rate_limit", ""), "auth.rateLimited");
    }

    #[tokio::test]
    async fn new_account_signs_up_then_verifies_the_code() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/signup"))
            .and(body_json(
                json!({ "email": "a@b.co", "password": "hunter22!" }),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({ "id": ME, "email": "a@b.co" })),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/verify"))
            .and(body_json(
                json!({ "type": "signup", "email": "a@b.co", "token": "123456" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(session_body("t1", false)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/my_account"))
            .and(header("authorization", "Bearer t1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "rank": 2, "banned": false, "ban_until": null, "ban_reason": null }
            ])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        assert!(!c.auth_status(false).await.signed_in);
        let r = c.sign_up(" A@B.co ", "hunter22!").await.unwrap();
        assert!(matches!(
            r,
            SignUpResult::CodeSent {
                purpose: CodePurpose::Signup
            }
        ));
        c.verify_code("a@b.co", "123 456", CodePurpose::Signup, "")
            .await
            .unwrap();
        let st = c.auth_status(true).await;
        assert!(st.signed_in && !st.offline);
        assert_eq!((st.email.as_deref(), st.rank), (Some("a@b.co"), 2));
        assert!(c.can_play().await);
        // Persisted with the cached rank.
        let again = FriendsClient::new(c.ctx().clone());
        let st = again.auth_status(false).await;
        assert!(st.signed_in && st.offline && st.rank == 2);
    }

    #[tokio::test]
    async fn anonymous_identity_is_upgraded_in_place() {
        let server = MockServer::builder().start().await;
        Mock::given(method("PUT"))
            .and(path("/auth/v1/user"))
            .and(header("authorization", "Bearer tok"))
            .and(body_json(json!({ "email": "a@b.co" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": ME })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/verify"))
            .and(body_json(
                json!({ "type": "email_change", "email": "a@b.co", "token": "654321" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(session_body("t2", true)))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/auth/v1/user"))
            .and(header("authorization", "Bearer t2"))
            .and(body_json(json!({ "password": "longpassword" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": ME })))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "refresh_token"))
            .and(body_json(json!({ "refresh_token": "r-t2" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(session_body("t3", false)))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = anonymous(&tmp, &server).await;
        let st = c.auth_status(true).await;
        assert!(!st.signed_in && st.anonymous_identity);
        assert!(!c.can_play().await);
        let r = c.sign_up("a@b.co", "longpassword").await.unwrap();
        assert!(matches!(
            r,
            SignUpResult::CodeSent {
                purpose: CodePurpose::Upgrade
            }
        ));
        c.verify_code("a@b.co", "654321", CodePurpose::Upgrade, "longpassword")
            .await
            .unwrap();
        let s = c.snapshot().await.unwrap();
        assert_eq!(s.user_id, ME, "same identity");
        assert!(!s.anonymous && s.friends_enabled, "friends consent kept");
        assert_eq!(s.access_token, "t3");
    }

    #[tokio::test]
    async fn password_reset_sets_the_new_password() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/recover"))
            .and(body_json(json!({ "email": "a@b.co" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/verify"))
            .and(body_partial_json(json!({ "type": "recovery" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(session_body("t4", false)))
            .mount(&server)
            .await;
        Mock::given(method("PUT"))
            .and(path("/auth/v1/user"))
            .and(body_json(json!({ "password": "newpassword" })))
            .respond_with(ResponseTemplate::new(422).set_body_json(
                json!({ "error_code": "same_password", "msg": "New password should be different" }),
            ))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        c.request_password_reset("a@b.co").await.unwrap();
        // Too short never reaches the server.
        assert_eq!(
            c.verify_code("a@b.co", "111111", CodePurpose::Recovery, "short")
                .await
                .unwrap_err()
                .code(),
            "auth.weakPassword"
        );
        c.verify_code("a@b.co", "111111", CodePurpose::Recovery, "newpassword")
            .await
            .unwrap();
        assert!(c.auth_status(false).await.signed_in);
    }

    #[tokio::test]
    async fn sign_in_replaces_an_anonymous_identity_and_reports_bans() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "password"))
            .and(body_partial_json(json!({ "password": "wrong" })))
            .respond_with(ResponseTemplate::new(400).set_body_json(
                json!({ "error_code": "invalid_credentials", "msg": "Invalid login credentials" }),
            ))
            .mount(&server)
            .await;
        let other = "99999999-9999-9999-9999-999999999999";
        let mut body = session_body("t5", false);
        body["user"]["id"] = json!(other);
        Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "password"))
            .and(body_partial_json(json!({ "password": "rightpass" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
        // The old anonymous identity is deleted with its own token.
        for p in [
            "/rest/v1/rpc/delete_me",
            "/storage/v1/object/list/textures",
            "/storage/v1/object/list/library",
            "/storage/v1/object/list/mods",
            "/storage/v1/object/list/avatars",
        ] {
            Mock::given(method("POST"))
                .and(path(p))
                .and(header("authorization", "Bearer tok"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
                .mount(&server)
                .await;
        }
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/my_account"))
            .and(header("authorization", "Bearer t5"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "rank": 0, "banned": true, "ban_until": "2026-12-01T00:00:00Z", "ban_reason": "hile" }
            ])))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = anonymous(&tmp, &server).await;
        assert_eq!(
            c.sign_in("a@b.co", "wrong").await.unwrap_err().code(),
            "auth.invalidCredentials"
        );
        c.sign_in("a@b.co", "rightpass").await.unwrap();
        assert_eq!(c.snapshot().await.unwrap().user_id, other);
        let st = c.auth_status(true).await;
        assert_eq!(
            st.ban,
            Some(BanInfo {
                until: Some("2026-12-01T00:00:00Z".into()),
                reason: "hile".into()
            })
        );
        assert!(!c.can_play().await);
    }

    #[tokio::test]
    async fn expired_session_signs_out_and_offline_keeps_the_cache() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error_code": "refresh_token_not_found", "msg": "Invalid Refresh Token: Refresh Token Not Found"
            })))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.save(&c.snapshot().await.unwrap()).unwrap();
        // A server error (no my_account yet) counts as offline.
        let st = c.auth_status(true).await;
        assert!(st.signed_in && st.offline);
        c.expire_token().await;
        let st = c.auth_status(true).await;
        assert!(!st.signed_in);
        assert!(!c.ctx().paths.friends_file().exists());
    }

    #[tokio::test]
    async fn sign_out_forgets_the_session() {
        let server = MockServer::builder().start().await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/logout"))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/rest/v1/rpc/go_offline"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = signed_in(&tmp, &server).await;
        c.save(&c.snapshot().await.unwrap()).unwrap();
        c.sign_out().await.unwrap();
        assert!(!c.has_identity().await);
        assert!(!c.ctx().paths.friends_file().exists());
        // Without a session nothing creates an anonymous identity.
        assert_eq!(
            c.ensure_identity().await.unwrap_err().code(),
            "auth.required"
        );
    }
}
