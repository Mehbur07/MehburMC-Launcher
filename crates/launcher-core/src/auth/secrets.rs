//! Secret storage for Microsoft refresh tokens.
//!
//! Production uses the OS credential store (Windows Credential Manager,
//! macOS Keychain, Secret Service) through `keyring`. Windows caps one
//! credential at 2560 bytes (UTF-16, ≈1280 characters) and Microsoft refresh
//! tokens can come close, so values are split into chunks:
//! `<key>` holds `chunks:<n>`, `<key>#<i>` hold the parts.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::{CoreError, Result};

pub const SERVICE: &str = "MehburMC Launcher";
const CHUNK_CHARS: usize = 1000;
const MAX_CHUNKS: usize = 16;

/// Minimal key/value secret store.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>>;
    fn set(&self, key: &str, value: &str) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
}

fn keyring_err(e: keyring::Error) -> CoreError {
    CoreError::Keyring(e.to_string())
}

/// The OS credential store.
pub struct KeyringStore;

impl KeyringStore {
    fn entry(key: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, key).map_err(keyring_err)
    }

    fn raw_get(key: &str) -> Result<Option<String>> {
        match Self::entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(keyring_err(e)),
        }
    }

    fn raw_delete(key: &str) -> Result<()> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(keyring_err(e)),
        }
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        let Some(head) = Self::raw_get(key)? else {
            return Ok(None);
        };
        let n = parse_header(&head)?;
        let mut out = String::new();
        for i in 0..n {
            match Self::raw_get(&format!("{key}#{i}"))? {
                Some(part) => out.push_str(&part),
                None => return Err(CoreError::Keyring(format!("chunk {i} of {key} is missing"))),
            }
        }
        Ok(Some(out))
    }

    fn set(&self, key: &str, value: &str) -> Result<()> {
        let parts = split(value);
        if parts.len() > MAX_CHUNKS {
            return Err(CoreError::Keyring("secret is too large".into()));
        }
        let old = Self::raw_get(key)?.and_then(|h| parse_header(&h).ok());
        for (i, p) in parts.iter().enumerate() {
            Self::entry(&format!("{key}#{i}"))?
                .set_password(p)
                .map_err(keyring_err)?;
        }
        Self::entry(key)?
            .set_password(&format!("chunks:{}", parts.len()))
            .map_err(keyring_err)?;
        // Drop leftovers of a longer previous value.
        if let Some(old) = old {
            for i in parts.len()..old {
                Self::raw_delete(&format!("{key}#{i}"))?;
            }
        }
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<()> {
        let n = Self::raw_get(key)?
            .and_then(|h| parse_header(&h).ok())
            .unwrap_or(MAX_CHUNKS);
        for i in 0..n {
            Self::raw_delete(&format!("{key}#{i}"))?;
        }
        Self::raw_delete(key)
    }
}

fn parse_header(h: &str) -> Result<usize> {
    h.strip_prefix("chunks:")
        .and_then(|n| n.parse().ok())
        .filter(|n| *n <= MAX_CHUNKS)
        .ok_or_else(|| CoreError::Keyring("unrecognised credential format".into()))
}

fn split(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(CHUNK_CHARS)
        .map(|c| c.iter().collect())
        .collect()
}

/// In-memory store (tests, or when the OS store is unavailable).
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.0.lock().expect("secrets").get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        self.0
            .lock()
            .expect("secrets")
            .insert(key.to_owned(), value.to_owned());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<()> {
        self.0.lock().expect("secrets").remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitting() {
        assert_eq!(split("").len(), 1);
        let long = "x".repeat(2500);
        let parts = split(&long);
        assert_eq!(parts.len(), 3);
        assert_eq!(parts.concat(), long);
        assert_eq!(parse_header("chunks:3").unwrap(), 3);
        assert!(parse_header("garbage").is_err());
        assert!(parse_header("chunks:99").is_err());
    }

    /// Touches the real credential store; run with `--ignored`.
    #[test]
    #[ignore]
    fn os_keyring_roundtrip() {
        let s = KeyringStore;
        let key = "mehburmc-selftest";
        let long = "M.C5".to_owned() + &"abcdefgh".repeat(400);
        s.set(key, &long).unwrap();
        assert_eq!(s.get(key).unwrap().as_deref(), Some(long.as_str()));
        s.set(key, "short").unwrap();
        assert_eq!(s.get(key).unwrap().as_deref(), Some("short"));
        s.delete(key).unwrap();
        assert_eq!(s.get(key).unwrap(), None);
    }
}
