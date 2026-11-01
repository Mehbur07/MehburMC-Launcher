//! `launcher/accounts.json` — offline account names and the selection.

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

/// `base`, or `base-2`, `base-3`, ... when an account already has it.
fn free_id(accounts: &[Account], base: &str) -> String {
    let free = |id: &str| !accounts.iter().any(|a| a.id == id);
    if free(base) {
        return base.to_owned();
    }
    (2u32..)
        .map(|n| format!("{base}-{n}"))
        .find(|id| free(id))
        .expect("there are fewer accounts than numbers")
}

/// Older builds could give two accounts one id. The first keeps it, along with
/// the selection, photo and skin; launching always found that one anyway.
fn split_shared_ids(accounts: &mut [Account]) {
    for i in 1..accounts.len() {
        if accounts[..i].iter().any(|a| a.id == accounts[i].id) {
            accounts[i].id = free_id(accounts, &accounts[i].id);
        }
    }
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

    /// Entries that no longer parse (e.g. Microsoft accounts from an older
    /// build) are skipped instead of discarding the whole file.
    pub fn view(&self) -> AccountsView {
        let Some(raw) = std::fs::read(self.paths.accounts_file())
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            return AccountsView::default();
        };
        let mut accounts: Vec<Account> = raw["accounts"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| serde_json::from_value(x.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        split_shared_ids(&mut accounts);
        let selected = raw["selected"]
            .as_str()
            .filter(|s| accounts.iter().any(|a| a.id == *s))
            .map(str::to_owned);
        AccountsView { accounts, selected }
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
                // A renamed account keeps the id of its first name, so this
                // name may already be in use as an id.
                let a = Account {
                    id: free_id(&v.accounts, &format!("offline-{}", uuid.replace('-', ""))),
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

    /// Renames an account. The in-game name and the offline UUID follow the
    /// new name; the id stays, so skin assignments and the selection survive.
    pub fn rename(&self, id: &str, name: &str) -> Result<Account> {
        let name = name.trim();
        validate_name(name)?;
        let _g = self.lock.lock().expect("accounts lock");
        let mut v = self.view();
        if v.accounts
            .iter()
            .any(|a| a.id != id && a.name.eq_ignore_ascii_case(name))
        {
            return Err(CoreError::AccountNameTaken(name.to_owned()));
        }
        let acc = v
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| CoreError::AccountNotFound(id.to_owned()))?;
        acc.name = name.to_owned();
        acc.uuid = offline_uuid(name);
        let out = acc.clone();
        self.write(&v)?;
        Ok(out)
    }

    pub fn remove(&self, id: &str) -> Result<AccountsView> {
        let _g = self.lock.lock().expect("accounts lock");
        let mut v = self.view();
        let before = v.accounts.len();
        v.accounts.retain(|a| a.id != id);
        if v.accounts.len() == before {
            return Err(CoreError::AccountNotFound(id.to_owned()));
        }
        if let Err(e) = super::avatar::AvatarStore::new(self.paths.clone()).clear(id) {
            tracing::warn!(error = %e.detail(), "could not delete the profile photo");
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

    #[test]
    fn rename_changes_name_and_uuid_but_keeps_id() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let s = AccountStore::new(paths);
        let a = s.add_offline("Steve").unwrap();
        let b = s.add_offline("Alex").unwrap();

        let r = s.rename(&a.id, " Notch ").unwrap();
        assert_eq!(r.id, a.id);
        assert_eq!(r.name, "Notch");
        assert_eq!(r.uuid, offline_uuid("Notch"));
        s.select(&a.id).unwrap();
        let launch = s.launch_account().unwrap();
        assert_eq!(launch.name, "Notch");
        assert_eq!(launch.uuid, r.uuid);

        // Case-only change of the same account is fine; another's name is not.
        assert_eq!(s.rename(&a.id, "notch").unwrap().name, "notch");
        assert_eq!(
            s.rename(&a.id, "alex").unwrap_err().code(),
            "account.nameTaken"
        );
        assert_eq!(s.rename(&b.id, "x").unwrap_err().code(), "auth.invalidName");
        assert_eq!(
            s.rename("nope", "Valid").unwrap_err().code(),
            "account.notFound"
        );
    }

    #[test]
    fn old_name_of_a_renamed_account_gets_its_own_id() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let s = AccountStore::new(paths);

        let renamed = s.add_offline("Steve").unwrap();
        s.rename(&renamed.id, "Notch").unwrap();
        let new = s.add_offline("Steve").unwrap();
        assert_ne!(new.id, renamed.id);
        assert_eq!(new.uuid, offline_uuid("Steve"));

        assert_eq!(s.launch_account().unwrap().name, "Steve");
        s.select(&renamed.id).unwrap();
        assert_eq!(s.launch_account().unwrap().name, "Notch");

        let v = s.remove(&new.id).unwrap();
        assert_eq!(v.accounts.len(), 1);
        assert_eq!(v.accounts[0].name, "Notch");
    }

    #[test]
    fn accounts_sharing_an_id_are_split() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        std::fs::write(
            paths.accounts_file(),
            r#"{"accounts":[
                {"id":"offline-a","kind":"offline","name":"Renamed","uuid":"u1","addedAt":1},
                {"id":"offline-b","kind":"offline","name":"Other","uuid":"u2","addedAt":2},
                {"id":"offline-a","kind":"offline","name":"FirstName","uuid":"u3","addedAt":3},
                {"id":"offline-a-2","kind":"offline","name":"Taken","uuid":"u4","addedAt":4}],
               "selected":"offline-a"}"#,
        )
        .unwrap();
        let s = AccountStore::new(paths);

        let v = s.view();
        let ids: Vec<&str> = v.accounts.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            ["offline-a", "offline-b", "offline-a-3", "offline-a-2"]
        );
        assert_eq!(v.selected.as_deref(), Some("offline-a"));
        assert_eq!(s.view(), v, "the same ids on every read");
        assert_eq!(s.launch_account().unwrap().name, "Renamed");

        // Each can now be used on its own, and a write makes the ids stick.
        s.select("offline-a-3").unwrap();
        assert_eq!(s.launch_account().unwrap().name, "FirstName");
        let raw = std::fs::read_to_string(s.paths.accounts_file()).unwrap();
        assert_eq!(raw.matches("\"offline-a\"").count(), 1);

        let v = s.remove("offline-a").unwrap();
        assert_eq!(v.accounts.len(), 3);
        assert_eq!(v.selected.as_deref(), Some("offline-a-3"));
    }

    #[test]
    fn legacy_microsoft_entries_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        std::fs::write(
            paths.accounts_file(),
            r#"{"accounts":[
                {"id":"msa-1","kind":"microsoft","name":"Notch","uuid":"u","addedAt":1},
                {"id":"offline-2","kind":"offline","name":"Steve","uuid":"v","addedAt":2}],
               "selected":"msa-1"}"#,
        )
        .unwrap();
        let s = AccountStore::new(paths);
        let v = s.view();
        assert_eq!(v.accounts.len(), 1);
        assert_eq!(v.accounts[0].name, "Steve");
        assert_eq!(
            v.selected, None,
            "selection of a removed account is dropped"
        );
    }
}
