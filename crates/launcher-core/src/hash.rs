//! File checksums (SHA-1 for Mojang/Maven, SHA-256 for Adoptium, SHA-512 for Modrinth).

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use sha1::{Digest, Sha1};
use sha2::{Sha256, Sha512};

use crate::error::{CoreError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checksum {
    Sha1(String),
    Sha256(String),
    Sha512(String),
}

impl Checksum {
    pub fn expected(&self) -> &str {
        match self {
            Self::Sha1(h) | Self::Sha256(h) | Self::Sha512(h) => h,
        }
    }

    /// Hashes `path` and compares case-insensitively.
    pub fn verify(&self, path: &Path) -> Result<()> {
        let actual = match self {
            Self::Sha1(_) => hash_file::<Sha1>(path)?,
            Self::Sha256(_) => hash_file::<Sha256>(path)?,
            Self::Sha512(_) => hash_file::<Sha512>(path)?,
        };
        if actual.eq_ignore_ascii_case(self.expected()) {
            Ok(())
        } else {
            Err(CoreError::HashMismatch {
                path: path.to_owned(),
                expected: self.expected().to_owned(),
                actual,
            })
        }
    }
}

fn hash_file<D: Digest>(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| CoreError::io(path, e))?;
    let mut reader = BufReader::with_capacity(256 * 1024, file);
    let mut hasher = D::new();
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| CoreError::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn sha1_file(path: &Path) -> Result<String> {
    hash_file::<Sha1>(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_known_digests() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(&p, b"abc").unwrap();
        Checksum::Sha1("a9993e364706816aba3e25717850c26c9cd0d89d".into())
            .verify(&p)
            .unwrap();
        Checksum::Sha256("BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD".into())
            .verify(&p)
            .unwrap();
        let err = Checksum::Sha1("00".into()).verify(&p).unwrap_err();
        assert_eq!(err.code(), "download.hashMismatch");
    }
}
