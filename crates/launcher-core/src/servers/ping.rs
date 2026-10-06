//! Server List Ping (1.7+ status protocol) over raw TCP: handshake → status
//! request → JSON response → ping/pong for the latency. SRV records
//! (`_minecraft._tcp.<host>`) are honoured like in the game when no port was
//! given.

use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use ts_rs::TS;

use super::motd::{self, MotdSpan};
use super::{MAX_ICON_B64, ServerAddress};
use crate::error::{CoreError, Result};

/// Whole exchange (DNS + connect + status), like the game's server list.
pub const TIMEOUT: Duration = Duration::from_secs(6);
const DNS_TIMEOUT: Duration = Duration::from_secs(3);
/// Status JSON with a favicon is a few tens of KiB; anything larger is junk.
const MAX_PACKET: usize = 1 << 20;
const MAX_SAMPLE: usize = 12;
/// Sent as "any version"; servers answer with their own protocol number.
const PROTOCOL_ANY: i32 = -1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ServerStatus {
    /// Server software / version text (`Paper 1.21.4`, `Velocity 3.x`).
    pub version: String,
    pub protocol: i32,
    pub players_online: u32,
    pub players_max: u32,
    /// A few online player names, if the server shares them.
    pub players_sample: Vec<String>,
    pub motd: Vec<MotdSpan>,
    /// `data:image/png;base64,…`
    pub icon: Option<String>,
    pub latency_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PingFailure {
    /// DNS failed or nothing listens on the port.
    Unreachable,
    Timeout,
    /// Something answered, but not a (1.7+) Minecraft server.
    BadResponse,
}

fn fail(address: &ServerAddress, kind: PingFailure) -> CoreError {
    CoreError::ServerPing {
        address: address.to_string(),
        kind,
    }
}

/// Resolves and pings `address`.
pub async fn ping(address: &ServerAddress) -> Result<ServerStatus> {
    match tokio::time::timeout(TIMEOUT, ping_inner(address)).await {
        Ok(r) => r,
        Err(_) => Err(fail(address, PingFailure::Timeout)),
    }
}

async fn ping_inner(address: &ServerAddress) -> Result<ServerStatus> {
    let targets = resolve(address).await;
    if targets.is_empty() {
        return Err(fail(address, PingFailure::Unreachable));
    }
    let mut last = PingFailure::Unreachable;
    for target in targets {
        match status_at(target, &address.host, address.port).await {
            Ok(s) => return Ok(s),
            Err(k) => {
                tracing::debug!(%target, ?k, "server ping failed");
                last = k;
                // A reply that is not a Minecraft status will not improve
                // on another address of the same server.
                if k == PingFailure::BadResponse {
                    break;
                }
            }
        }
    }
    Err(fail(address, last))
}

/// Socket addresses to try, SRV target first.
pub async fn resolve(address: &ServerAddress) -> Vec<SocketAddr> {
    if let Ok(ip) = address.host.parse::<IpAddr>() {
        return vec![SocketAddr::new(ip, address.port)];
    }
    let (host, port) = match address.explicit_port {
        true => (address.host.clone(), address.port),
        false => srv(&address.host)
            .await
            .unwrap_or_else(|| (address.host.clone(), address.port)),
    };
    let lookup = tokio::time::timeout(DNS_TIMEOUT, tokio::net::lookup_host((host.as_str(), port)));
    match lookup.await {
        Ok(Ok(addrs)) => {
            // IPv4 first: many hosts publish AAAA records they do not serve.
            let mut v: Vec<SocketAddr> = addrs.collect();
            v.sort_by_key(|a| a.is_ipv6());
            v.dedup();
            v
        }
        _ => Vec::new(),
    }
}

/// `_minecraft._tcp.<host>` → `(target, port)`, if published.
async fn srv(host: &str) -> Option<(String, u16)> {
    use hickory_resolver::TokioResolver;
    use hickory_resolver::proto::rr::RData;

    let resolver = TokioResolver::builder_tokio().ok()?.build().ok()?;
    let name = format!("_minecraft._tcp.{}.", host.trim_end_matches('.'));
    let lookup = tokio::time::timeout(DNS_TIMEOUT, resolver.srv_lookup(name))
        .await
        .ok()?
        .ok()?;
    let mut records: Vec<_> = lookup
        .answers()
        .iter()
        .filter_map(|r| match &r.data {
            RData::SRV(s) => Some((s.priority, s.weight, s.target.to_utf8(), s.port)),
            _ => None,
        })
        .collect();
    // Lowest priority wins; highest weight among equals.
    records.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let (_, _, target, port) = records.into_iter().next()?;
    let target = target.trim_end_matches('.').to_owned();
    (!target.is_empty() && port != 0).then_some((target, port))
}

fn put_varint(out: &mut Vec<u8>, v: i32) {
    let mut v = v as u32;
    loop {
        if v & !0x7F == 0 {
            out.push(v as u8);
            return;
        }
        out.push((v as u8 & 0x7F) | 0x80);
        v >>= 7;
    }
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(body.len() + 5);
    put_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::with_capacity(inner.len() + 5);
    put_varint(&mut out, inner.len() as i32);
    out.extend_from_slice(&inner);
    out
}

fn handshake(host: &str, port: u16) -> Vec<u8> {
    let mut body = Vec::new();
    put_varint(&mut body, PROTOCOL_ANY);
    put_varint(&mut body, host.len() as i32);
    body.extend_from_slice(host.as_bytes());
    body.extend_from_slice(&port.to_be_bytes());
    put_varint(&mut body, 1); // next state: status
    packet(0x00, &body)
}

async fn read_varint<R: AsyncReadExt + Unpin>(r: &mut R) -> std::io::Result<i32> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let b = r.read_u8().await?;
        value |= u32::from(b & 0x7F) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "varint too long",
    ))
}

/// One packet: `(id, body)`.
async fn read_packet<R: AsyncReadExt + Unpin>(r: &mut R) -> std::io::Result<(i32, Vec<u8>)> {
    let len = read_varint(r).await?;
    let len = usize::try_from(len)
        .ok()
        .filter(|l| (1..=MAX_PACKET).contains(l))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad length"))?;
    let mut buf = vec![0; len];
    r.read_exact(&mut buf).await?;
    let mut cur = std::io::Cursor::new(buf);
    let id = read_varint(&mut cur).await?;
    let start = cur.position() as usize;
    let mut buf = cur.into_inner();
    buf.drain(..start);
    Ok((id, buf))
}

/// Status exchange with one socket address. `host`/`port` go into the
/// handshake as typed by the user (proxies route by them).
pub async fn status_at(
    target: SocketAddr,
    host: &str,
    port: u16,
) -> std::result::Result<ServerStatus, PingFailure> {
    let stream = TcpStream::connect(target)
        .await
        .map_err(|_| PingFailure::Unreachable)?;
    let _ = stream.set_nodelay(true);
    let mut s = BufReader::new(stream);
    let bad = |_| PingFailure::BadResponse;

    let mut out = handshake(host, port);
    out.extend_from_slice(&packet(0x00, &[]));
    let sent = Instant::now();
    s.get_mut().write_all(&out).await.map_err(bad)?;

    let (id, body) = read_packet(&mut s).await.map_err(bad)?;
    let status_rtt = sent.elapsed();
    if id != 0x00 {
        return Err(PingFailure::BadResponse);
    }
    let mut cur = std::io::Cursor::new(body.as_slice());
    let json_len = read_varint(&mut cur).await.map_err(bad)?;
    let start = cur.position() as usize;
    let json = usize::try_from(json_len)
        .ok()
        .and_then(|l| body.get(start..start.checked_add(l)?))
        .ok_or(PingFailure::BadResponse)?;
    let value: Value = serde_json::from_slice(json).map_err(|_| PingFailure::BadResponse)?;

    // Latency from ping/pong; some servers close right after the status,
    // then the status round trip is the best estimate.
    let token = 0x4D45_4842_5552_i64; // "MEHBUR"
    let ping_sent = Instant::now();
    let rtt = match s
        .get_mut()
        .write_all(&packet(0x01, &token.to_be_bytes()))
        .await
    {
        Ok(()) => match tokio::time::timeout(Duration::from_secs(2), read_packet(&mut s)).await {
            Ok(Ok((0x01, b))) if b == token.to_be_bytes() => ping_sent.elapsed(),
            _ => status_rtt,
        },
        Err(_) => status_rtt,
    };
    let mut status = parse_status(&value).ok_or(PingFailure::BadResponse)?;
    status.latency_ms = rtt.as_millis().min(u128::from(u32::MAX)) as u32;
    Ok(status)
}

/// The status JSON; `None` if it is not one.
pub fn parse_status(v: &Value) -> Option<ServerStatus> {
    let obj = v.as_object()?;
    if !obj.contains_key("version") && !obj.contains_key("description") {
        return None;
    }
    let num = |v: Option<&Value>| {
        v.and_then(Value::as_i64)
            .map(|n| n.clamp(0, i64::from(u32::MAX)) as u32)
            .unwrap_or(0)
    };
    let players = obj.get("players");
    let sample = players
        .and_then(|p| p.get("sample"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| p.get("name")?.as_str())
                .map(|n| {
                    let spans = motd::parse(&Value::String(n.to_owned()));
                    motd::plain(&spans).chars().take(48).collect::<String>()
                })
                .filter(|n| !n.trim().is_empty())
                .take(MAX_SAMPLE)
                .collect()
        })
        .unwrap_or_default();
    let version = obj.get("version");
    let icon = obj
        .get("favicon")
        .and_then(Value::as_str)
        .map(|s| s.replace(['\n', '\r'], ""))
        .filter(|s| {
            s.strip_prefix("data:image/png;base64,").is_some_and(|b| {
                !b.is_empty()
                    && b.len() <= MAX_ICON_B64
                    && b.bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'='))
            })
        });
    Some(ServerStatus {
        version: version
            .and_then(|v| v.get("name"))
            .and_then(Value::as_str)
            .map(|n| motd::plain(&motd::parse(&Value::String(n.to_owned()))))
            .unwrap_or_default()
            .chars()
            .take(64)
            .collect(),
        protocol: version
            .and_then(|v| v.get("protocol"))
            .and_then(Value::as_i64)
            .and_then(|p| i32::try_from(p).ok())
            .unwrap_or(-1),
        players_online: num(players.and_then(|p| p.get("online"))),
        players_max: num(players.and_then(|p| p.get("max"))),
        players_sample: sample,
        motd: obj.get("description").map(motd::parse).unwrap_or_default(),
        icon,
        latency_ms: 0,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn varint_encoding() {
        let enc = |v| {
            let mut o = Vec::new();
            put_varint(&mut o, v);
            o
        };
        assert_eq!(enc(0), [0x00]);
        assert_eq!(enc(127), [0x7f]);
        assert_eq!(enc(128), [0x80, 0x01]);
        assert_eq!(enc(25565), [0xdd, 0xc7, 0x01]);
        assert_eq!(enc(-1), [0xff, 0xff, 0xff, 0xff, 0x0f]);
    }

    #[test]
    fn status_json() {
        let s = parse_status(&json!({
            "version": {"name": "§6Paper 1.21.4", "protocol": 769},
            "players": {"max": 100, "online": 7,
                        "sample": [{"name": "Steve", "id": "x"}, {"name": "§aAlex"}]},
            "description": {"text": "Merhaba", "color": "aqua"},
            "favicon": "data:image/png;base64,iVBORw0KGgo="
        }))
        .unwrap();
        assert_eq!(s.version, "Paper 1.21.4");
        assert_eq!((s.protocol, s.players_online, s.players_max), (769, 7, 100));
        assert_eq!(s.players_sample, ["Steve", "Alex"]);
        assert_eq!(motd::plain(&s.motd), "Merhaba");
        assert!(s.icon.is_some());

        // Non-PNG / script favicons are dropped; negatives clamp to 0.
        let s = parse_status(&json!({
            "description": "x",
            "players": {"online": -5},
            "favicon": "javascript:alert(1)"
        }))
        .unwrap();
        assert_eq!((s.players_online, s.icon), (0, None));
        assert!(parse_status(&json!({"hello": 1})).is_none());
        assert!(parse_status(&json!([1])).is_none());
    }

    /// A fake server speaking the status protocol on localhost.
    async fn fake_server(answer_ping: bool, reply: Vec<u8>) -> SocketAddr {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            let (sock, _) = l.accept().await.unwrap();
            let mut s = BufReader::new(sock);
            let (id, hs) = read_packet(&mut s).await.unwrap();
            assert_eq!(id, 0);
            // protocol -1 (5 bytes), host len 9 "localhost", port, state 1
            assert_eq!(&hs[5..15], b"\x09localhost");
            assert_eq!(*hs.last().unwrap(), 1);
            let (id, _) = read_packet(&mut s).await.unwrap();
            assert_eq!(id, 0);
            s.get_mut().write_all(&reply).await.unwrap();
            if answer_ping {
                let (id, body) = read_packet(&mut s).await.unwrap();
                assert_eq!(id, 1);
                s.get_mut().write_all(&packet(1, &body)).await.unwrap();
            }
        });
        addr
    }

    fn status_packet(json: &str) -> Vec<u8> {
        let mut body = Vec::new();
        put_varint(&mut body, json.len() as i32);
        body.extend_from_slice(json.as_bytes());
        packet(0, &body)
    }

    #[tokio::test]
    async fn pings_a_fake_server() {
        let json = r#"{"version":{"name":"1.21.4","protocol":769},"players":{"max":20,"online":3},"description":"§bHi"}"#;
        for answer_ping in [true, false] {
            let addr = fake_server(answer_ping, status_packet(json)).await;
            let s = status_at(addr, "localhost", 25565).await.unwrap();
            assert_eq!((s.players_online, s.players_max), (3, 20));
            assert_eq!(s.motd[0].color.as_deref(), Some("#55ffff"));
        }
    }

    #[tokio::test]
    async fn rejects_non_minecraft_replies() {
        let addr = fake_server(false, b"HTTP/1.1 400 Bad Request\r\n\r\n".to_vec()).await;
        assert_eq!(
            status_at(addr, "localhost", 25565).await,
            Err(PingFailure::BadResponse)
        );
        let addr = fake_server(false, status_packet("not json")).await;
        assert_eq!(
            status_at(addr, "localhost", 25565).await,
            Err(PingFailure::BadResponse)
        );
    }

    #[tokio::test]
    async fn closed_port_is_unreachable() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        assert_eq!(
            status_at(addr, "localhost", addr.port()).await,
            Err(PingFailure::Unreachable)
        );
        let a = ServerAddress::parse(&format!("127.0.0.1:{}", addr.port())).unwrap();
        assert_eq!(ping(&a).await.unwrap_err().code(), "server.unreachable");
    }
}
