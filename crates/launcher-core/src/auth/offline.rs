//! Offline accounts: name validation and Mojang's offline UUID
//! (`UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`, an MD5-based v3 UUID).

use md5::{Digest, Md5};

use super::LaunchAccount;
use crate::error::{CoreError, Result};

pub fn validate_name(name: &str) -> Result<()> {
    let ok = (3..=16).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(())
    } else {
        Err(CoreError::InvalidPlayerName(name.to_owned()))
    }
}

/// Hyphenated offline UUID, identical to what offline-mode servers compute.
pub fn offline_uuid(name: &str) -> String {
    let mut b: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    b[6] = (b[6] & 0x0f) | 0x30; // version 3
    b[8] = (b[8] & 0x3f) | 0x80; // IETF variant
    let h = hex::encode(b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

impl LaunchAccount {
    pub fn offline(name: &str) -> Result<Self> {
        validate_name(name)?;
        Ok(Self {
            name: name.to_owned(),
            uuid: offline_uuid(name),
            // Any non-empty value; offline servers never check it.
            access_token: "0".into(),
            user_type: "legacy".into(),
            xuid: "0".into(),
            client_id: "0".into(),
            offline: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_java_name_uuid_from_bytes() {
        assert_eq!(
            offline_uuid("Notch"),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
        assert_eq!(
            offline_uuid("Steve"),
            "5627dd98-e6be-3c21-b8a8-e92344183641"
        );
    }

    #[test]
    fn name_rules() {
        for ok in ["Steve", "abc", "A_b_9", "sixteen_chars_xx"] {
            validate_name(ok).unwrap();
        }
        for bad in ["ab", "seventeen_chars_x", "bad name", "türkçe", "a-b", ""] {
            assert_eq!(
                validate_name(bad).unwrap_err().code(),
                "auth.invalidName",
                "{bad}"
            );
        }
    }

    #[test]
    fn offline_account() {
        let a = LaunchAccount::offline("Steve").unwrap();
        assert!(a.offline);
        assert_eq!(a.uuid_simple(), "5627dd98e6be3c21b8a8e92344183641");
        assert_eq!(a.session(), "-");
    }
}
