//! Microsoft account sign-in for Minecraft: Java Edition.
//!
//! ```text
//! device code ──poll──▶ MSA access + refresh token
//!   └─▶ Xbox Live user token (XBL) ─▶ XSTS token (rp://api.minecraftservices.com/)
//!        └─▶ login_with_xbox ─▶ Minecraft access token ─▶ /minecraft/profile
//! ```
//!
//! Only the refresh token is persisted (OS keyring); every other token lives
//! in memory. Nothing token-bearing is ever logged or put into an error.

use std::time::{Duration, Instant};

use base64::Engine;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};

pub const SCOPE: &str = "XboxLive.signin offline_access";
const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

/// Microsoft OAuth tokens.
#[derive(Clone, Deserialize)]
pub struct MsaTokens {
    pub access_token: String,
    pub refresh_token: String,
}

impl std::fmt::Debug for MsaTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MsaTokens(***)")
    }
}

/// A signed-in Minecraft session.
#[derive(Clone)]
pub struct McSession {
    pub access_token: String,
    pub expires_at: Instant,
    /// Hyphenated UUID.
    pub uuid: String,
    pub name: String,
    pub xuid: String,
}

impl std::fmt::Debug for McSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McSession")
            .field("name", &self.name)
            .field("uuid", &self.uuid)
            .finish_non_exhaustive()
    }
}

impl McSession {
    /// Still usable for at least five more minutes.
    pub fn is_fresh(&self) -> bool {
        self.expires_at > Instant::now() + Duration::from_secs(300)
    }
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
}

fn token_url(ctx: &Ctx) -> String {
    format!("{}/consumers/oauth2/v2.0/token", ctx.endpoints.ms_login)
}

fn parse<T: for<'de> Deserialize<'de>>(step: &str, status: u16, body: &[u8]) -> Result<T> {
    serde_json::from_slice(body).map_err(|_| CoreError::AuthFailed {
        step: step.into(),
        status,
    })
}

pub async fn request_device_code(ctx: &Ctx, client_id: &str) -> Result<DeviceCode> {
    let url = format!(
        "{}/consumers/oauth2/v2.0/devicecode",
        ctx.endpoints.ms_login
    );
    let (status, body) = ctx
        .http
        .post_form(&url, &[("client_id", client_id), ("scope", SCOPE)])
        .await?;
    if status != 200 {
        tracing::warn!(status, "device code request rejected");
        return Err(CoreError::AuthFailed {
            step: "devicecode".into(),
            status,
        });
    }
    parse("devicecode", status, &body)
}

/// Polls until the user finished (or abandoned) the sign-in in the browser.
pub async fn poll_device_code(
    ctx: &Ctx,
    client_id: &str,
    code: &DeviceCode,
    cancel: &CancellationToken,
) -> Result<MsaTokens> {
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval.max(1));
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Err(CoreError::Cancelled),
            _ = tokio::time::sleep(interval) => {}
        }
        if Instant::now() >= deadline {
            return Err(CoreError::AuthCodeExpired);
        }
        let (status, body) = ctx
            .http
            .post_form(
                &token_url(ctx),
                &[
                    ("grant_type", DEVICE_CODE_GRANT),
                    ("client_id", client_id),
                    ("device_code", &code.device_code),
                ],
            )
            .await?;
        if status == 200 {
            return parse("token", status, &body);
        }
        let err: OAuthError = parse("token", status, &body)?;
        match err.error.as_str() {
            "authorization_pending" => {}
            "slow_down" => interval += Duration::from_secs(5),
            "authorization_declined" | "access_denied" => return Err(CoreError::AuthDeclined),
            "expired_token" | "code_expired" | "bad_verification_code" => {
                return Err(CoreError::AuthCodeExpired);
            }
            other => {
                tracing::warn!(error = other, "device code polling failed");
                return Err(CoreError::AuthFailed {
                    step: "token".into(),
                    status,
                });
            }
        }
    }
}

/// Exchanges a refresh token for new tokens. An invalid/revoked token is
/// reported as [`CoreError::AuthRelogin`] (name filled in by the caller).
pub async fn refresh(ctx: &Ctx, client_id: &str, refresh_token: &str) -> Result<MsaTokens> {
    let (status, body) = ctx
        .http
        .post_form(
            &token_url(ctx),
            &[
                ("grant_type", "refresh_token"),
                ("client_id", client_id),
                ("refresh_token", refresh_token),
                ("scope", SCOPE),
            ],
        )
        .await?;
    if status == 200 {
        return parse("refresh", status, &body);
    }
    let err = serde_json::from_slice::<OAuthError>(&body).map(|e| e.error);
    match err.as_deref() {
        Ok("invalid_grant") | Ok("interaction_required") | Ok("invalid_client") => {
            Err(CoreError::AuthRelogin {
                name: String::new(),
            })
        }
        _ => Err(CoreError::AuthFailed {
            step: "refresh".into(),
            status,
        }),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxResponse {
    token: String,
    display_claims: XboxClaims,
}

#[derive(Debug, Deserialize)]
struct XboxClaims {
    xui: Vec<XboxUser>,
}

#[derive(Debug, Deserialize)]
struct XboxUser {
    uhs: String,
}

#[derive(Debug, Deserialize)]
struct XErr {
    #[serde(rename = "XErr")]
    xerr: u64,
}

#[derive(Debug, Deserialize)]
struct McLogin {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct Profile {
    id: String,
    name: String,
}

/// Turns MSA tokens into a Minecraft session (XBL → XSTS → Minecraft).
pub async fn minecraft_login(ctx: &Ctx, msa_access_token: &str) -> Result<McSession> {
    let e = &ctx.endpoints;
    // 1. Xbox Live user token.
    let (status, body) = ctx
        .http
        .post_json(
            &format!("{}/user/authenticate", e.xbox_user),
            &json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": format!("d={msa_access_token}"),
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT",
            }),
            None,
        )
        .await?;
    if status != 200 {
        return Err(CoreError::AuthFailed {
            step: "xbl".into(),
            status,
        });
    }
    let xbl: XboxResponse = parse("xbl", status, &body)?;

    // 2. XSTS token for Minecraft services.
    let (status, body) = ctx
        .http
        .post_json(
            &format!("{}/xsts/authorize", e.xbox_xsts),
            &json!({
                "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
                "RelyingParty": "rp://api.minecraftservices.com/",
                "TokenType": "JWT",
            }),
            None,
        )
        .await?;
    if status == 401 {
        let code = serde_json::from_slice::<XErr>(&body)
            .map(|x| x.xerr)
            .unwrap_or(0);
        return Err(CoreError::AuthXbox { code });
    }
    if status != 200 {
        return Err(CoreError::AuthFailed {
            step: "xsts".into(),
            status,
        });
    }
    let xsts: XboxResponse = parse("xsts", status, &body)?;
    let uhs = xsts
        .display_claims
        .xui
        .first()
        .map(|u| u.uhs.clone())
        .ok_or_else(|| CoreError::AuthFailed {
            step: "xsts".into(),
            status,
        })?;

    // 3. Minecraft access token.
    let (status, body) = ctx
        .http
        .post_json(
            &format!("{}/authentication/login_with_xbox", e.mc_services),
            &json!({ "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token) }),
            None,
        )
        .await?;
    // Apps without Mojang's approval get 403 ("Invalid app registration").
    if status == 403 {
        return Err(CoreError::AuthNotApproved);
    }
    if status != 200 {
        return Err(CoreError::AuthFailed {
            step: "minecraft".into(),
            status,
        });
    }
    let login: McLogin = parse("minecraft", status, &body)?;

    // 4. Profile (404 = does not own the game / no name chosen yet).
    let (status, body) = ctx
        .http
        .get_bearer(
            &format!("{}/minecraft/profile", e.mc_services),
            &login.access_token,
        )
        .await?;
    if status == 404 {
        return Err(CoreError::AuthNoProfile);
    }
    if status != 200 {
        return Err(CoreError::AuthFailed {
            step: "profile".into(),
            status,
        });
    }
    let profile: Profile = parse("profile", status, &body)?;
    Ok(McSession {
        xuid: jwt_claim(&login.access_token, "xuid").unwrap_or_else(|| "0".into()),
        access_token: login.access_token,
        expires_at: Instant::now() + Duration::from_secs(login.expires_in),
        uuid: hyphenate(&profile.id),
        name: profile.name,
    })
}

/// Verification pages we are willing to open in the browser.
pub fn is_microsoft_page(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && matches!(
                u.host_str(),
                Some(
                    "www.microsoft.com"
                        | "microsoft.com"
                        | "login.live.com"
                        | "login.microsoftonline.com"
                        | "aka.ms"
                )
            )
    })
}

/// Reads a string claim from a JWT payload (no signature check needed: the
/// token came straight from Minecraft services over TLS).
pub fn jwt_claim(token: &str, claim: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    match &v[claim] {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// `069a79f444e94726a5befca90e38aaf5` → `069a79f4-44e9-4726-a5be-fca90e38aaf5`.
pub fn hyphenate(id: &str) -> String {
    if id.len() != 32 || id.contains('-') {
        return id.to_owned();
    }
    format!(
        "{}-{}-{}-{}-{}",
        &id[0..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..32]
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;

    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::events::NullSink;
    use crate::net::{Allowlist, Http};
    use crate::paths::Paths;

    pub(crate) fn mock_ctx(tmp: &std::path::Path, server: &MockServer) -> Ctx {
        let paths = Paths::at(tmp.join("MehburMC"));
        paths.ensure_layout().unwrap();
        let mut ctx = Ctx::new(paths, Arc::new(NullSink), 4).unwrap();
        ctx.http = Http::new(Allowlist::with_loopback()).unwrap();
        let uri = server.uri();
        ctx.endpoints.ms_login = uri.clone();
        ctx.endpoints.xbox_user = uri.clone();
        ctx.endpoints.xbox_xsts = uri.clone();
        ctx.endpoints.mc_services = uri;
        ctx
    }

    /// A JWT-shaped token whose payload carries `xuid`.
    pub(crate) fn mc_token() -> String {
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"xuid":"2535400000000001","sub":"x"}"#);
        format!("eyJhbGciOiJIUzI1NiJ9.{payload}.sig")
    }

    /// Mounts a successful XBL → XSTS → Minecraft → profile chain.
    pub(crate) async fn mount_chain(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .and(body_string_contains("d=msa-access"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xbl-token", "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/xsts/authorize"))
            .and(body_string_contains("xbl-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xsts-token", "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
            })))
            .mount(server)
            .await;
        Mock::given(method("POST"))
            .and(path("/authentication/login_with_xbox"))
            .and(body_string_contains("XBL3.0 x=uhs1;xsts-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": mc_token(), "expires_in": 86400, "username": "x"
            })))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/minecraft/profile"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "069a79f444e94726a5befca90e38aaf5", "name": "Notch", "skins": [], "capes": []
            })))
            .mount(server)
            .await;
    }

    #[test]
    fn helpers() {
        assert_eq!(
            hyphenate("069a79f444e94726a5befca90e38aaf5"),
            "069a79f4-44e9-4726-a5be-fca90e38aaf5"
        );
        assert_eq!(
            jwt_claim(&mc_token(), "xuid").as_deref(),
            Some("2535400000000001")
        );
        assert_eq!(jwt_claim("garbage", "xuid"), None);
        assert!(is_microsoft_page("https://www.microsoft.com/link"));
        assert!(!is_microsoft_page("http://www.microsoft.com/link"));
        assert!(!is_microsoft_page("https://microsoft.com.evil.io/link"));
        let s = format!(
            "{:?}",
            MsaTokens {
                access_token: "secret-a".into(),
                refresh_token: "secret-r".into()
            }
        );
        assert!(!s.contains("secret"));
    }

    #[tokio::test]
    async fn device_code_flow_and_chain() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/devicecode"))
            .and(body_string_contains("client_id=cid"))
            .and(body_string_contains("scope=XboxLive.signin+offline_access"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code": "dev", "user_code": "ABCD-EFGH",
                "verification_uri": "https://www.microsoft.com/link", "expires_in": 900, "interval": 1
            })))
            .mount(&server)
            .await;
        // First poll: pending, then success.
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/token"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"error": "authorization_pending"})),
            )
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/token"))
            .and(body_string_contains("device_code=dev"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "msa-access", "refresh_token": "msa-refresh", "expires_in": 3600
            })))
            .mount(&server)
            .await;
        mount_chain(&server).await;

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mock_ctx(tmp.path(), &server);
        let code = request_device_code(&ctx, "cid").await.unwrap();
        assert_eq!(code.user_code, "ABCD-EFGH");
        let tokens = poll_device_code(&ctx, "cid", &code, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(tokens.refresh_token, "msa-refresh");
        let s = minecraft_login(&ctx, &tokens.access_token).await.unwrap();
        assert_eq!(s.name, "Notch");
        assert_eq!(s.uuid, "069a79f4-44e9-4726-a5be-fca90e38aaf5");
        assert_eq!(s.xuid, "2535400000000001");
        assert!(s.is_fresh());
    }

    #[tokio::test]
    async fn declined_and_errors_are_mapped() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/token"))
            .and(body_string_contains("grant_type=urn"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({"error": "authorization_declined"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"error": "invalid_grant"})),
            )
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xbl", "DisplayClaims": {"xui": [{"uhs": "u"}]}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/xsts/authorize"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "Identity": "0", "XErr": 2148916238u64, "Message": ""
            })))
            .mount(&server)
            .await;

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mock_ctx(tmp.path(), &server);
        let code = DeviceCode {
            device_code: "d".into(),
            user_code: "u".into(),
            verification_uri: "https://www.microsoft.com/link".into(),
            expires_in: 60,
            interval: 1,
        };
        let e = poll_device_code(&ctx, "cid", &code, &CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(e.code(), "auth.declined");
        assert_eq!(
            refresh(&ctx, "cid", "old").await.unwrap_err().code(),
            "auth.relogin"
        );
        let e = minecraft_login(&ctx, "msa").await.unwrap_err();
        assert_eq!(e.code(), "auth.xbox.child");
        assert!(!e.detail().contains("msa"), "no tokens in error details");
    }

    #[tokio::test]
    async fn unapproved_app_and_missing_profile() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xbl", "DisplayClaims": {"xui": [{"uhs": "u"}]}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/xsts/authorize"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xsts", "DisplayClaims": {"xui": [{"uhs": "u"}]}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/authentication/login_with_xbox"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Invalid app registration"))
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mock_ctx(tmp.path(), &server);
        assert_eq!(
            minecraft_login(&ctx, "m").await.unwrap_err().code(),
            "auth.notApproved"
        );

        server.reset().await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xbl", "DisplayClaims": {"xui": [{"uhs": "u"}]}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/xsts/authorize"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xsts", "DisplayClaims": {"xui": [{"uhs": "u"}]}
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/authentication/login_with_xbox"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": mc_token(), "expires_in": 86400
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/minecraft/profile"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({"error": "NOT_FOUND"})))
            .mount(&server)
            .await;
        assert_eq!(
            minecraft_login(&ctx, "m").await.unwrap_err().code(),
            "auth.noProfile"
        );
    }
}
