//! Multiplayer servers: launcher favourites (`launcher/servers.json`), the
//! game's own list per instance (`servers.dat`) and status pings.

pub mod dat;
pub mod motd;
pub mod nbt;
pub mod ping;

use std::fmt;
use std::net::Ipv6Addr;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use ts_rs::TS;

use crate::error::{CoreError, Result};
use crate::fsutil::write_json_atomic;
use crate::instance::now_secs;
use crate::paths::Paths;

pub const DEFAULT_PORT: u16 = 25565;
/// Base64 length of a 64×64 server icon is ~10 KiB; allow generous headroom.
pub const MAX_ICON_B64: usize = 128 * 1024;
const MAX_NAME_LEN: usize = 64;
const MAX_FAVORITES: usize = 200;

/// `host[:port]`, validated. Hosts are compared case-insensitively.
#[derive(Debug, Clone, Eq)]
pub struct ServerAddress {
    /// Lower-case host name or IP (IPv6 without brackets).
    pub host: String,
    pub port: u16,
    /// The port was typed; SRV records are only used without one.
    pub explicit_port: bool,
}

impl PartialEq for ServerAddress {
    fn eq(&self, other: &Self) -> bool {
        self.host == other.host && self.port == other.port
    }
}

impl fmt::Display for ServerAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == DEFAULT_PORT {
            f.write_str(&host)
        } else {
            write!(f, "{host}:{}", self.port)
        }
    }
}

impl ServerAddress {
    pub fn parse(input: &str) -> Result<Self> {
        let s = input.trim();
        let bad = || CoreError::ServerAddressInvalid(input.trim().chars().take(80).collect());
        if s.is_empty() || s.len() > 261 {
            return Err(bad());
        }
        let (host, port) = if let Some(rest) = s.strip_prefix('[') {
            let (h, after) = rest.split_once(']').ok_or_else(bad)?;
            h.parse::<Ipv6Addr>().map_err(|_| bad())?;
            match after {
                "" => (h, None),
                p => (h, Some(p.strip_prefix(':').ok_or_else(bad)?)),
            }
        } else if s.matches(':').count() > 1 {
            // Bare IPv6 without a port.
            s.parse::<Ipv6Addr>().map_err(|_| bad())?;
            (s, None)
        } else {
            match s.split_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (s, None),
            }
        };
        let port = match port {
            None => None,
            Some(p) => Some(p.parse::<u16>().ok().filter(|p| *p != 0).ok_or_else(bad)?),
        };
        if !host.contains(':') && !valid_host(host) {
            return Err(bad());
        }
        Ok(Self {
            host: host.to_ascii_lowercase(),
            port: port.unwrap_or(DEFAULT_PORT),
            explicit_port: port.is_some(),
        })
    }

    /// `--server`/`--port` or `--quickPlayMultiplayer` value.
    pub fn connect_string(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("{host}:{}", self.port)
    }
}

/// DNS name or IPv4 literal: letters, digits, `-`, `_` (SRV-style), dots.
fn valid_host(h: &str) -> bool {
    let h = h.strip_suffix('.').unwrap_or(h);
    !h.is_empty()
        && h.len() <= 253
        && h.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

fn clean_name(name: &str, address: &ServerAddress) -> Result<String> {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).collect();
    if name.chars().count() > MAX_NAME_LEN {
        return Err(CoreError::InvalidSetting("server name".into()));
    }
    Ok(if name.is_empty() {
        address.to_string()
    } else {
        name
    })
}

/// An entry of the game's multiplayer list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameServer {
    /// Position in `servers.dat` (needed to remove it).
    pub index: u32,
    pub name: String,
    pub address: String,
    /// Icon the game cached, as a data URI.
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FavoriteServer {
    /// Derived from the normalised address, so each server is listed once.
    pub id: String,
    pub name: String,
    pub address: String,
    #[ts(type = "number")]
    pub added_at: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct FavoritesFile {
    servers: Vec<FavoriteServer>,
}

pub fn favorite_id(address: &ServerAddress) -> String {
    let digest = Sha1::digest(address.connect_string().as_bytes());
    hex::encode(&digest[..6])
}

/// `launcher/servers.json`.
pub struct ServerStore {
    paths: Paths,
    lock: Mutex<()>,
}

impl ServerStore {
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            lock: Mutex::new(()),
        }
    }

    fn file(&self) -> std::path::PathBuf {
        self.paths.launcher_dir().join("servers.json")
    }

    /// Entries that no longer parse are skipped.
    pub fn favorites(&self) -> Vec<FavoriteServer> {
        let Some(raw) = std::fs::read(self.file())
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        else {
            return Vec::new();
        };
        raw["servers"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| serde_json::from_value::<FavoriteServer>(x.clone()).ok())
                    .filter(|f| ServerAddress::parse(&f.address).is_ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn write(&self, servers: Vec<FavoriteServer>) -> Result<Vec<FavoriteServer>> {
        let file = FavoritesFile { servers };
        write_json_atomic(&self.file(), &file)?;
        Ok(file.servers)
    }

    /// Adds a favourite; the same address again only renames it.
    pub fn add(&self, name: &str, address: &str) -> Result<Vec<FavoriteServer>> {
        let addr = ServerAddress::parse(address)?;
        let name = clean_name(name, &addr)?;
        let id = favorite_id(&addr);
        let _g = self.lock.lock().expect("servers lock");
        let mut list = self.favorites();
        match list.iter_mut().find(|f| f.id == id) {
            Some(f) => f.name = name,
            None => {
                if list.len() >= MAX_FAVORITES {
                    return Err(CoreError::InvalidSetting("servers".into()));
                }
                list.push(FavoriteServer {
                    id,
                    name,
                    address: addr.to_string(),
                    added_at: now_secs(),
                });
            }
        }
        self.write(list)
    }

    pub fn remove(&self, id: &str) -> Result<Vec<FavoriteServer>> {
        let _g = self.lock.lock().expect("servers lock");
        let mut list = self.favorites();
        let before = list.len();
        list.retain(|f| f.id != id);
        if list.len() == before {
            return Err(CoreError::ServerNotFound(id.to_owned()));
        }
        self.write(list)
    }

    /// New order by id; unknown ids are ignored, missing ones keep their
    /// relative order at the end.
    pub fn reorder(&self, ids: &[String]) -> Result<Vec<FavoriteServer>> {
        let _g = self.lock.lock().expect("servers lock");
        let mut list = self.favorites();
        list.sort_by_key(|f| ids.iter().position(|i| *i == f.id).unwrap_or(usize::MAX));
        self.write(list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_addresses() {
        let a = ServerAddress::parse(" Play.Example.org ").unwrap();
        assert_eq!(
            (a.host.as_str(), a.port, a.explicit_port),
            ("play.example.org", 25565, false)
        );
        assert_eq!(a.to_string(), "play.example.org");
        assert_eq!(a.connect_string(), "play.example.org:25565");

        let a = ServerAddress::parse("1.2.3.4:25570").unwrap();
        assert_eq!((a.port, a.explicit_port), (25570, true));
        assert_eq!(a.to_string(), "1.2.3.4:25570");

        let a = ServerAddress::parse("[::1]:25566").unwrap();
        assert_eq!((a.host.as_str(), a.port), ("::1", 25566));
        assert_eq!(a.to_string(), "[::1]:25566");
        assert_eq!(
            ServerAddress::parse("::1").unwrap().connect_string(),
            "[::1]:25565"
        );

        // Default port written out is the same server.
        assert_eq!(
            ServerAddress::parse("a.b").unwrap(),
            ServerAddress::parse("A.B:25565").unwrap()
        );

        for bad in [
            "",
            "   ",
            "host:",
            "host:0",
            "host:70000",
            "host:abc",
            "ho st",
            "-a.com",
            "a..b",
            "[::1",
            "[nope]:1",
            "a:b:c",
            "http://x.com",
            "x.com/path",
            "şş.com",
        ] {
            assert!(
                ServerAddress::parse(bad).is_err(),
                "{bad:?} should be rejected"
            );
        }
        assert_eq!(
            ServerAddress::parse("x y").unwrap_err().code(),
            "server.addressInvalid"
        );
    }

    #[test]
    fn favorites_store() {
        let d = tempfile::tempdir().unwrap();
        let store = ServerStore::new(Paths::at(d.path()));
        assert!(store.favorites().is_empty());

        let l = store.add("  Hypixel ", "mc.hypixel.net").unwrap();
        assert_eq!(l[0].name, "Hypixel");
        // Same server again (different spelling) only renames.
        let l = store.add("Hypixel!", "MC.HYPIXEL.NET:25565").unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "Hypixel!");
        // Empty name falls back to the address.
        let l = store.add("", "localhost:25570").unwrap();
        assert_eq!(l[1].name, "localhost:25570");

        assert!(store.add("x", "bad host").is_err());
        assert!(store.add(&"n".repeat(65), "a.b").is_err());

        let ids: Vec<String> = l.iter().rev().map(|f| f.id.clone()).collect();
        let l = store.reorder(&ids).unwrap();
        assert_eq!(l[0].address, "localhost:25570");

        let l = store.remove(&l[0].id).unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(store.remove("nope").unwrap_err().code(), "server.notFound");
        assert_eq!(store.favorites(), l);
    }
}
