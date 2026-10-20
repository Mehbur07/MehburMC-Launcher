//! "Sign in with Google" (K83): Supabase OAuth with PKCE. The browser signs
//! in at Google and comes back to a one-shot listener on 127.0.0.1; the
//! launcher then trades the code (plus its secret verifier) for a session.
//! The launcher never sees the Google password.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::Method;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use super::{ANON_KEY, FriendsClient};
use crate::error::{CoreError, Result};

/// Fixed so it can be listed in Supabase → Authentication → URL
/// Configuration → Redirect URLs.
pub const CALLBACK_PORT: u16 = 47823;
pub const CALLBACK_PATH: &str = "/auth/callback";
/// How long the browser may take.
const WAIT: Duration = Duration::from_secs(300);

pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}{CALLBACK_PATH}")
}

fn new_verifier() -> Result<String> {
    let mut bytes = [0u8; 48];
    getrandom::fill(&mut bytes).map_err(|e| {
        tracing::warn!(error = %e, "no randomness for the PKCE verifier");
        CoreError::Friends("auth.googleFailed")
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[derive(Debug, PartialEq)]
enum Callback {
    Code(String),
    Error(String),
}

/// `GET /auth/callback?code=… HTTP/1.1` → the code or the provider's error.
fn parse_callback(request_line: &str) -> Option<Callback> {
    let target = request_line.split_whitespace().nth(1)?;
    let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
    if url.path() != CALLBACK_PATH {
        return None;
    }
    let (mut code, mut error) = (None, None);
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "error_description" => error = Some(v.into_owned()),
            "error" if error.is_none() => error = Some(v.into_owned()),
            _ => {}
        }
    }
    code.map(Callback::Code).or(error.map(Callback::Error))
}

const PAGE: &str = "<!doctype html><meta charset=utf-8><title>MehburMC</title>\
<body style=\"font-family:sans-serif;background:#0b0f14;color:#e6f1ff;display:grid;place-items:center;height:100vh;margin:0\">\
<div style=\"text-align:center\"><h2>{title}</h2><p>{body}</p></div>";

async fn respond(sock: &mut TcpStream, status: &str, title: &str, body: &str) {
    let html = PAGE.replace("{title}", title).replace("{body}", body);
    let msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
        html.len()
    );
    let _ = sock.write_all(msg.as_bytes()).await;
    let _ = sock.shutdown().await;
}

async fn wait_for_code(listener: TcpListener, cancel: &CancellationToken) -> Result<String> {
    let deadline = tokio::time::sleep(WAIT);
    tokio::pin!(deadline);
    loop {
        let mut sock = tokio::select! {
            r = listener.accept() => match r {
                Ok((sock, _)) => sock,
                Err(e) => {
                    tracing::debug!(error = %e, "callback accept failed");
                    continue;
                }
            },
            () = cancel.cancelled() => return Err(CoreError::Friends("auth.googleCancelled")),
            () = &mut deadline => return Err(CoreError::Friends("auth.googleTimeout")),
        };
        let mut buf = vec![0u8; 8192];
        let n = tokio::time::timeout(Duration::from_secs(5), sock.read(&mut buf))
            .await
            .ok()
            .and_then(std::result::Result::ok)
            .unwrap_or(0);
        let request = String::from_utf8_lossy(&buf[..n]);
        match parse_callback(request.lines().next().unwrap_or("")) {
            Some(Callback::Code(code)) => {
                respond(
                    &mut sock,
                    "200 OK",
                    "Giriş tamamlandı / Signed in",
                    "Launcher'a dönebilirsin. / You can go back to the launcher.",
                )
                .await;
                return Ok(code);
            }
            Some(Callback::Error(e)) => {
                tracing::warn!(error = %e, "Google sign-in was refused");
                respond(
                    &mut sock,
                    "200 OK",
                    "Giriş yapılamadı / Sign-in failed",
                    "Launcher'a dönüp tekrar dene. / Go back to the launcher and try again.",
                )
                .await;
                return Err(CoreError::Friends("auth.googleFailed"));
            }
            // favicon.ico and other strays.
            None => respond(&mut sock, "404 Not Found", "", "").await,
        }
    }
}

impl FriendsClient {
    /// Signs in with Google: `open` shows the given URL in the browser.
    /// Ends after the browser comes back, on [`Self::cancel_google_sign_in`]
    /// or after five minutes.
    pub async fn google_sign_in(&self, open: impl FnOnce(&str) -> Result<()>) -> Result<()> {
        self.google_sign_in_on(CALLBACK_PORT, open).await
    }

    /// Stops a running [`Self::google_sign_in`].
    pub fn cancel_google_sign_in(&self) {
        if let Some(t) = self.google.lock().expect("google lock").take() {
            t.cancel();
        }
    }

    pub(crate) async fn google_sign_in_on(
        &self,
        port: u16,
        open: impl FnOnce(&str) -> Result<()>,
    ) -> Result<()> {
        if !self.google_enabled().await? {
            return Err(CoreError::Friends("auth.googleDisabled"));
        }
        let listener = TcpListener::bind(("127.0.0.1", port)).await.map_err(|e| {
            tracing::warn!(error = %e, port, "Google callback port unavailable");
            CoreError::Friends("auth.googlePortBusy")
        })?;
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let verifier = new_verifier()?;
        let mut url = reqwest::Url::parse(&self.url("/auth/v1/authorize")).map_err(|_| {
            CoreError::UrlNotAllowed {
                url: self.url("/auth/v1/authorize"),
            }
        })?;
        url.query_pairs_mut()
            .append_pair("provider", "google")
            .append_pair("redirect_to", &redirect_uri(port))
            .append_pair("code_challenge", &challenge(&verifier))
            .append_pair("code_challenge_method", "s256");

        let cancel = CancellationToken::new();
        if let Some(old) = self
            .google
            .lock()
            .expect("google lock")
            .replace(cancel.clone())
        {
            old.cancel();
        }
        open(url.as_str())?;
        let code = wait_for_code(listener, &cancel).await;
        self.google.lock().expect("google lock").take();
        let s = self
            .auth_call(
                "/auth/v1/token?grant_type=pkce",
                json!({ "auth_code": code?, "code_verifier": verifier }),
            )
            .await?;
        self.finish_sign_in(s).await
    }

    /// Google is switched on in Supabase (public auth settings).
    async fn google_enabled(&self) -> Result<bool> {
        let (status, bytes) = self
            .ctx()
            .http
            .request_json(
                Method::GET,
                &self.url("/auth/v1/settings"),
                None,
                &[("apikey", ANON_KEY)],
            )
            .await?;
        if !(200..300).contains(&status) {
            return Err(super::server_error(status, &bytes));
        }
        let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        Ok(v["external"]["google"].as_bool().unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use wiremock::matchers::{body_partial_json, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::tests::ctx;
    use super::*;

    #[test]
    fn reads_the_callback() {
        assert_eq!(
            parse_callback("GET /auth/callback?code=abc-123&x=1 HTTP/1.1"),
            Some(Callback::Code("abc-123".into()))
        );
        assert_eq!(
            parse_callback(
                "GET /auth/callback?error=access_denied&error_description=Nope HTTP/1.1"
            ),
            Some(Callback::Error("Nope".into()))
        );
        assert_eq!(parse_callback("GET /favicon.ico HTTP/1.1"), None);
        assert_eq!(parse_callback(""), None);
    }

    #[test]
    fn challenge_is_the_s256_of_the_verifier() {
        // base64url(SHA-256), cross-checked with Node's crypto.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mJ0kzTSvYP_4Rjk0dMAOH2xZ2tf7Tc"),
            "hmx6oobGb0wJ-zw12IKDMrsRIkL_jt4-yfnowL_DDuQ"
        );
        let v = new_verifier().unwrap();
        assert_eq!(v.len(), 64);
        assert_ne!(v, new_verifier().unwrap());
    }

    async fn settings(server: &MockServer, google: bool) {
        Mock::given(method("GET"))
            .and(path("/auth/v1/settings"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "external": { "google": google, "email": true } })),
            )
            .mount(server)
            .await;
    }

    /// Plays the browser: follows `redirect_to` back with `query`.
    fn browser(seen: Arc<Mutex<String>>, query: &'static str) -> impl FnOnce(&str) -> Result<()> {
        move |url: &str| {
            *seen.lock().unwrap() = url.to_owned();
            let u = reqwest::Url::parse(url).unwrap();
            let back = u
                .query_pairs()
                .find(|(k, _)| k == "redirect_to")
                .unwrap()
                .1
                .into_owned();
            tokio::spawn(async move {
                let _ = reqwest::get(format!("{back}?{query}")).await;
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn signs_in_with_the_code_and_verifier() {
        let server = MockServer::builder().start().await;
        settings(&server, true).await;
        Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "pkce"))
            .and(body_partial_json(json!({ "auth_code": "the-code" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "acc", "refresh_token": "ref", "expires_in": 3600,
                "user": { "id": "u-google", "email": "g@gmail.com", "is_anonymous": false }
            })))
            .expect(1)
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let c = FriendsClient::new(ctx(&tmp, &server));
        c.create_local_account().await.unwrap();
        let seen = Arc::new(Mutex::new(String::new()));
        c.google_sign_in_on(0, browser(seen.clone(), "code=the-code"))
            .await
            .unwrap();

        let st = c.auth_status(false).await;
        assert!(st.signed_in && !st.local_account);
        assert_eq!(st.email.as_deref(), Some("g@gmail.com"));

        // The verifier sent matches the challenge the browser carried.
        let opened = reqwest::Url::parse(&seen.lock().unwrap()).unwrap();
        let q = |k: &str| {
            opened
                .query_pairs()
                .find(|(n, _)| n == k)
                .unwrap()
                .1
                .into_owned()
        };
        assert_eq!(q("provider"), "google");
        assert_eq!(q("code_challenge_method"), "s256");
        let token_req = server
            .received_requests()
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.url.path() == "/auth/v1/token")
            .unwrap();
        let body: Value = serde_json::from_slice(&token_req.body).unwrap();
        assert_eq!(
            challenge(body["code_verifier"].as_str().unwrap()),
            q("code_challenge")
        );
    }

    #[tokio::test]
    async fn reports_a_switched_off_provider_a_refusal_and_cancel() {
        let server = MockServer::builder().start().await;
        let tmp = tempfile::tempdir().unwrap();
        let c = Arc::new(FriendsClient::new(ctx(&tmp, &server)));

        settings(&server, false).await;
        let never = |_: &str| -> Result<()> { panic!("browser opened") };
        assert_eq!(
            c.google_sign_in_on(0, never).await.unwrap_err().code(),
            "auth.googleDisabled"
        );

        server.reset().await;
        settings(&server, true).await;
        let seen = Arc::new(Mutex::new(String::new()));
        let err = c
            .google_sign_in_on(0, browser(seen, "error=access_denied"))
            .await
            .unwrap_err();
        assert_eq!(err.code(), "auth.googleFailed");

        let c2 = c.clone();
        let waiting = tokio::spawn(async move { c2.google_sign_in_on(0, |_| Ok(())).await });
        tokio::time::sleep(Duration::from_millis(200)).await;
        c.cancel_google_sign_in();
        assert_eq!(
            waiting.await.unwrap().unwrap_err().code(),
            "auth.googleCancelled"
        );
        assert!(!c.auth_status(false).await.signed_in);
    }
}
