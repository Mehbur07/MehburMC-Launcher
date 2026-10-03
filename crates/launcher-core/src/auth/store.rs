//! `launcher/accounts.json` — account metadata only. Secrets (Microsoft
//! refresh tokens, phase 5) go to the OS keyring, never into this file.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::LaunchAccount;
use super::offline::{offline_uuid, validate_name};
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
}

impl AccountStore {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            lock: Mutex::new(()),
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

    /// Adds (or returns the existing) offline account and selects it.
    pub fn add_offline(&self, name: &str) -> Result<Account> {
        let name = name.trim();
        validate_name(name)?;
        let _g = self.lock.lock().expect("accounts lock");
        let mut v = self.view();
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
                };
                v.accounts.push(a.clone());
                a
            }
        };
        v.selected = Some(account.id.clone());
        self.write(&v)?;
        Ok(account)
    }

    pub fn remove(&self, id: &str) -> Result<AccountsView> {
        let _g = self.lock.lock().expect("accounts lock");
        let mut v = self.view();
        let before = v.accounts.len();
        v.accounts.retain(|a| a.id != id);
        if v.accounts.len() == before {
            return Err(CoreError::AccountNotFound(id.to_owned()));
        }
        if v.selected.as_deref() == Some(id) {
            v.selected = v.accounts.first().map(|a| a.id.clone());
        }
        self.write(&v)?;
        Ok(v)
    }

    pub fn select(&self, id: &str) -> Result<AccountsView> {
        let _g = self.lock.lock().expect("accounts lock");
        let mut v = self.view();
        if !v.accounts.iter().any(|a| a.id == id) {
            return Err(CoreError::AccountNotFound(id.to_owned()));
        }
        v.selected = Some(id.to_owned());
        self.write(&v)?;
        Ok(v)
    }

    /// Identity for the selected account.
    pub fn launch_account(&self) -> Result<LaunchAccount> {
        let v = self.view();
        let id = v.selected.ok_or(CoreError::NoAccount)?;
        let acc = v
            .accounts
            .into_iter()
            .find(|a| a.id == id)
            .ok_or(CoreError::NoAccount)?;
        match acc.kind {
            AccountKind::Offline => LaunchAccount::offline(&acc.name),
            // Microsoft sign-in arrives in phase 5.
            AccountKind::Microsoft => Err(CoreError::NoAccount),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_accounts_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let s = AccountStore::new(paths);

        assert_eq!(s.launch_account().unwrap_err().code(), "account.none");
        let a = s.add_offline("Steve").unwrap();
        assert_eq!(a.uuid, "5627dd98-e6be-3c21-b8a8-e92344183641");
        let again = s.add_offline("steve").unwrap();
        assert_eq!(again.id, a.id, "same name is not duplicated");
        let b = s.add_offline("Alex").unwrap();
        assert_eq!(s.view().selected.as_deref(), Some(b.id.as_str()));
        assert_eq!(s.launch_account().unwrap().name, "Alex");

        s.select(&a.id).unwrap();
        assert_eq!(s.launch_account().unwrap().name, "Steve");
        let v = s.remove(&a.id).unwrap();
        assert_eq!(v.selected.as_deref(), Some(b.id.as_str()));
        assert!(s.add_offline("x").is_err());
        assert!(s.select("nope").is_err());
    }
}
