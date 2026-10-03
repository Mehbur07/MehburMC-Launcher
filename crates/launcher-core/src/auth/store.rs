//! `launcher/accounts.json` — account metadata only. Microsoft refresh
//! tokens go to the OS keyring ([`SecretStore`]); Minecraft access tokens are
//! cached in memory for the lifetime of the process.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::LaunchAccount;
use super::microsoft::{self, McSession};
use super::offline::{offline_uuid, validate_name};
use super::secrets::SecretStore;
use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::fsutil::write_json_atomic;
use crate::instance::now_secs;
use crate::paths::Paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AccountKind {
    Offline,
    Microsoft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Account {
    pub id: String,
    pub kind: AccountKind,
    pub name: String,
    /// Hyphenated UUID.
    pub uuid: String,
    #[ts(type = "number")]
    pub added_at: u64,
    /// Microsoft only: the stored session is no longer valid.
    #[serde(default)]
    pub needs_login: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AccountsView {
    pub accounts: Vec<Account>,
    pub selected: Option<String>,
}

pub struct AccountStore {
    paths: Paths,
    lock: Mutex<()>,
    secrets: Arc<dyn SecretStore>,
    /// account id → Minecraft session (memory only).
    sessions: Mutex<HashMap<String, McSession>>,
}

fn msa_id(uuid: &str) -> String {
    format!("msa-{}", uuid.replace('-', ""))
}

impl AccountStore {
    pub fn new(paths: Paths, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            paths,
            lock: Mutex::new(()),
            secrets,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn view(&self) -> AccountsView {
        std::fs::read(self.paths.accounts_file())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn write(&self, v: &AccountsView) -> Result<()> {
        write_json_atomic(&self.paths.accounts_file(), v)
    }

    fn modify<T>(&self, f: impl FnOnce(&mut AccountsView) -> Result<T>) -> Result<T> {
        let _g = self.lock.lock().expect("accounts lock");
        let mut v = self.view();
        let out = f(&mut v)?;
        self.write(&v)?;
        Ok(out)
    }

    /// Adds (or returns the existing) offline account and selects it.
    pub fn add_offline(&self, name: &str) -> Result<Account> {
        let name = name.trim();
        validate_name(name)?;
        self.modify(|v| {
            let existing = v
                .accounts
                .iter()
                .find(|a| a.kind == AccountKind::Offline && a.name.eq_ignore_ascii_case(name))
                .cloned();
            let account = match existing {
                Some(a) => a,
                None => {
                    let uuid = offline_uuid(name);
                    let a = Account {
                        id: format!("offline-{}", uuid.replace('-', "")),
                        kind: AccountKind::Offline,
                        name: name.to_owned(),
                        uuid,
                        added_at: now_secs(),
                        needs_login: false,
                    };
                    v.accounts.push(a.clone());
                    a
                }
            };
            v.selected = Some(account.id.clone());
            Ok(account)
        })
    }

    /// Stores a freshly signed-in Microsoft account (or refreshes an existing
    /// one with the same UUID) and selects it.
    pub fn add_microsoft(&self, session: McSession, refresh_token: &str) -> Result<Account> {
        let id = msa_id(&session.uuid);
        self.secrets.set(&id, refresh_token)?;
        let account = self.modify(|v| {
            let account = match v.accounts.iter_mut().find(|a| a.id == id) {
                Some(a) => {
                    a.name = session.name.clone();
                    a.needs_login = false;
                    a.clone()
                }
                None => {
                    let a = Account {
                        id: id.clone(),
                        kind: AccountKind::Microsoft,
                        name: session.name.clone(),
                        uuid: session.uuid.clone(),
                        added_at: now_secs(),
                        needs_login: false,
                    };
                    v.accounts.push(a.clone());
                    a
                }
            };
            v.selected = Some(id.clone());
            Ok(account)
        })?;
        self.sessions.lock().expect("sessions").insert(id, session);
        tracing::info!(name = %account.name, "Microsoft account signed in");
        Ok(account)
    }

    pub fn remove(&self, id: &str) -> Result<AccountsView> {
        let removed = self.modify(|v| {
            let pos = v
                .accounts
                .iter()
                .position(|a| a.id == id)
                .ok_or_else(|| CoreError::AccountNotFound(id.to_owned()))?;
            let a = v.accounts.remove(pos);
            if v.selected.as_deref() == Some(id) {
                v.selected = v.accounts.first().map(|a| a.id.clone());
            }
            Ok(a)
        })?;
        if removed.kind == AccountKind::Microsoft {
            self.sessions.lock().expect("sessions").remove(id);
            if let Err(e) = self.secrets.delete(id) {
                tracing::warn!(error = %e.detail(), "could not delete stored credential");
            }
        }
        Ok(self.view())
    }

    pub fn select(&self, id: &str) -> Result<AccountsView> {
        self.modify(|v| {
            if !v.accounts.iter().any(|a| a.id == id) {
                return Err(CoreError::AccountNotFound(id.to_owned()));
            }
            v.selected = Some(id.to_owned());
            Ok(())
        })?;
        Ok(self.view())
    }

    /// The selected account.
    pub fn selected(&self) -> Result<Account> {
        let v = self.view();
        let id = v.selected.ok_or(CoreError::NoAccount)?;
        v.accounts
            .into_iter()
            .find(|a| a.id == id)
            .ok_or(CoreError::NoAccount)
    }

    fn set_needs_login(&self, id: &str, value: bool) {
        let r = self.modify(|v| {
            if let Some(a) = v.accounts.iter_mut().find(|a| a.id == id) {
                a.needs_login = value;
            }
            Ok(())
        });
        if let Err(e) = r {
            tracing::warn!(error = %e.detail(), "could not update account state");
        }
    }

    /// Identity for `acc`, refreshing Microsoft tokens as needed. Without a
    /// network connection a Microsoft account still launches with its last
    /// known name and UUID (single player only).
    pub async fn launch_account(
        &self,
        ctx: &Ctx,
        acc: &Account,
        client_id: Option<&str>,
    ) -> Result<LaunchAccount> {
        if acc.kind == AccountKind::Offline {
            return LaunchAccount::offline(&acc.name);
        }
        let cached = self
            .sessions
            .lock()
            .expect("sessions")
            .get(&acc.id)
            .filter(|s| s.is_fresh())
            .cloned();
        let session = match cached {
            Some(s) => s,
            None => match self.refresh_session(ctx, acc, client_id).await {
                Ok(s) => s,
                // Network down or Microsoft/Xbox outage (5xx, 429).
                Err(e) if e.is_transient() => {
                    tracing::warn!(error = %e.detail(), "Microsoft services unreachable, launching offline");
                    return Ok(offline_msa(acc));
                }
                Err(e) => return Err(e),
            },
        };
        Ok(LaunchAccount {
            name: session.name.clone(),
            uuid: session.uuid.clone(),
            access_token: session.access_token.clone(),
            user_type: "msa".into(),
            xuid: session.xuid.clone(),
            client_id: client_id.unwrap_or("0").to_owned(),
            offline: false,
        })
    }

    async fn refresh_session(
        &self,
        ctx: &Ctx,
        acc: &Account,
        client_id: Option<&str>,
    ) -> Result<McSession> {
        let client_id = client_id.ok_or(CoreError::AuthNotConfigured)?;
        let relogin = || CoreError::AuthRelogin {
            name: acc.name.clone(),
        };
        let Some(refresh_token) = self.secrets.get(&acc.id)? else {
            self.set_needs_login(&acc.id, true);
            return Err(relogin());
        };
        let tokens = match microsoft::refresh(ctx, client_id, &refresh_token).await {
            Ok(t) => t,
            Err(CoreError::AuthRelogin { .. }) => {
                self.set_needs_login(&acc.id, true);
                return Err(relogin());
            }
            Err(e) => return Err(e),
        };
        // Microsoft rotates refresh tokens; keep the newest.
        self.secrets.set(&acc.id, &tokens.refresh_token)?;
        let session = microsoft::minecraft_login(ctx, &tokens.access_token).await?;
        if session.name != acc.name || acc.needs_login {
            let (id, name) = (acc.id.clone(), session.name.clone());
            self.modify(|v| {
                if let Some(a) = v.accounts.iter_mut().find(|a| a.id == id) {
                    a.name = name;
                    a.needs_login = false;
                }
                Ok(())
            })?;
        }
        self.sessions
            .lock()
            .expect("sessions")
            .insert(acc.id.clone(), session.clone());
        Ok(session)
    }
}

/// Microsoft identity without a valid token (no network).
fn offline_msa(acc: &Account) -> LaunchAccount {
    LaunchAccount {
        name: acc.name.clone(),
        uuid: acc.uuid.clone(),
        access_token: "0".into(),
        user_type: "msa".into(),
        xuid: "0".into(),
        client_id: "0".into(),
        offline: true,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::auth::microsoft::tests::{mock_ctx, mount_chain};
    use crate::auth::secrets::MemoryStore;

    fn store(tmp: &std::path::Path) -> (AccountStore, Arc<MemoryStore>) {
        let paths = Paths::at(tmp.join("MehburMC"));
        paths.ensure_layout().unwrap();
        let secrets = Arc::new(MemoryStore::default());
        (AccountStore::new(paths, secrets.clone()), secrets)
    }

    fn session(name: &str, fresh: bool) -> McSession {
        McSession {
            access_token: "mc-token".into(),
            expires_at: if fresh {
                Instant::now() + Duration::from_secs(3600)
            } else {
                Instant::now()
            },
            uuid: "069a79f4-44e9-4726-a5be-fca90e38aaf5".into(),
            name: name.into(),
            xuid: "1".into(),
        }
    }

    #[tokio::test]
    async fn offline_accounts_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let (s, _) = store(tmp.path());
        let server = MockServer::start().await;
        let ctx = mock_ctx(tmp.path(), &server);

        assert_eq!(s.selected().unwrap_err().code(), "account.none");
        let a = s.add_offline("Steve").unwrap();
        assert_eq!(a.uuid, "5627dd98-e6be-3c21-b8a8-e92344183641");
        let again = s.add_offline("steve").unwrap();
        assert_eq!(again.id, a.id, "same name is not duplicated");
        let b = s.add_offline("Alex").unwrap();
        assert_eq!(s.view().selected.as_deref(), Some(b.id.as_str()));
        let acc = s.selected().unwrap();
        assert_eq!(
            s.launch_account(&ctx, &acc, None).await.unwrap().name,
            "Alex"
        );

        s.select(&a.id).unwrap();
        assert_eq!(s.selected().unwrap().name, "Steve");
        let v = s.remove(&a.id).unwrap();
        assert_eq!(v.selected.as_deref(), Some(b.id.as_str()));
        assert!(s.add_offline("x").is_err());
        assert!(s.select("nope").is_err());
    }

    #[tokio::test]
    async fn microsoft_account_refresh_and_removal() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/token"))
            .and(body_string_contains("refresh_token=old-refresh"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "msa-access", "refresh_token": "new-refresh"
            })))
            .expect(1)
            .mount(&server)
            .await;
        mount_chain(&server).await;
        let tmp = tempfile::tempdir().unwrap();
        let (s, secrets) = store(tmp.path());
        let ctx = mock_ctx(tmp.path(), &server);

        // Expired session forces a refresh on launch.
        let acc = s
            .add_microsoft(session("OldName", false), "old-refresh")
            .unwrap();
        assert_eq!(acc.id, "msa-069a79f444e94726a5befca90e38aaf5");
        assert!(
            !std::fs::read_to_string(ctx.paths.accounts_file())
                .unwrap()
                .contains("refresh"),
            "no secrets in accounts.json"
        );
        let la = s.launch_account(&ctx, &acc, Some("cid")).await.unwrap();
        assert_eq!(la.name, "Notch");
        assert_eq!(la.user_type, "msa");
        assert!(!la.offline);
        assert_eq!(
            secrets.get(&acc.id).unwrap().as_deref(),
            Some("new-refresh")
        );
        assert_eq!(s.selected().unwrap().name, "Notch", "name synced");

        // Second launch uses the cached session (expect(1) above).
        let acc = s.selected().unwrap();
        s.launch_account(&ctx, &acc, Some("cid")).await.unwrap();

        s.remove(&acc.id).unwrap();
        assert_eq!(secrets.get(&acc.id).unwrap(), None);
    }

    #[tokio::test]
    async fn revoked_token_needs_login_and_offline_fallback() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/consumers/oauth2/v2.0/token"))
            .respond_with(
                ResponseTemplate::new(400).set_body_json(json!({"error": "invalid_grant"})),
            )
            .mount(&server)
            .await;
        let tmp = tempfile::tempdir().unwrap();
        let (s, _) = store(tmp.path());
        let ctx = mock_ctx(tmp.path(), &server);
        let acc = s.add_microsoft(session("Notch", false), "revoked").unwrap();
        let e = s.launch_account(&ctx, &acc, Some("cid")).await.unwrap_err();
        assert_eq!(e.code(), "auth.relogin");
        assert_eq!(e.params()["name"], "Notch");
        assert!(s.selected().unwrap().needs_login);
        assert_eq!(
            s.launch_account(&ctx, &acc, None).await.unwrap_err().code(),
            "auth.notConfigured"
        );

        // Server gone → offline launch with the last known identity.
        let acc = s.add_microsoft(session("Notch", false), "valid").unwrap();
        let mut down = ctx.clone();
        down.endpoints.ms_login = "http://127.0.0.1:1".into();
        let la = s.launch_account(&down, &acc, Some("cid")).await.unwrap();
        assert!(la.offline);
        assert_eq!(la.uuid, "069a79f4-44e9-4726-a5be-fca90e38aaf5");
        assert_eq!(la.user_type, "msa");
    }
}
