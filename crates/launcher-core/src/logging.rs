//! Logging: daily-rotated files under `logs/`, secrets masked before writing.

use std::io::{self, Write};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Number of daily log files kept.
const MAX_LOG_FILES: usize = 7;

static SECRET_PATTERNS: LazyLock<Vec<(Regex, &'static str)>> = LazyLock::new(|| {
    vec![
        // JSON Web Tokens (Minecraft access tokens, XSTS tokens).
        (
            Regex::new(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*").unwrap(),
            "***",
        ),
        // Microsoft refresh/access tokens (e.g. `M.C5xx_BAY.0.U.-...`).
        (
            Regex::new(r"\bM\.[A-Z0-9]{1,6}_[A-Za-z0-9!*$.\-_]{16,}").unwrap(),
            "***",
        ),
        // CurseForge API keys (bcrypt-shaped: `$2a$10$` + 53 chars).
        (
            Regex::new(r"\$2[aby]\$\d{2}\$[./A-Za-z0-9]{53}").unwrap(),
            "***",
        ),
        // HTTP authorization headers, including the scheme word.
        (
            Regex::new(r#"(?i)(authorization["']?\s*[:=]\s*["']?)(?:bearer\s+)?[^\s"',}&]+"#)
                .unwrap(),
            "${1}***",
        ),
        // key=value / key: value / --key value forms.
        (
            Regex::new(
                r#"(?i)((?:--)?(?:access_?token|refresh_?token|refreshToken|bearer|auth_access_token|xuid|clientsecret|client_secret|password|x-api-key|api_?key|curseforge_?api_?key)["']?\s*[:= ]\s*["']?)[^\s"',}&]+"#,
            )
            .unwrap(),
            "${1}***",
        ),
    ]
});

/// Replaces anything that looks like a credential with `***`.
pub fn mask_secrets(input: &str) -> String {
    let mut out = input.to_owned();
    for (re, rep) in SECRET_PATTERNS.iter() {
        if re.is_match(&out) {
            out = re.replace_all(&out, *rep).into_owned();
        }
    }
    out
}

/// Initializes the global subscriber. Keep the returned guard alive for the
/// lifetime of the process, otherwise buffered log lines are lost.
pub fn init(logs_dir: &Path, debug: bool) -> io::Result<WorkerGuard> {
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("launcher")
        .filename_suffix("log")
        .max_log_files(MAX_LOG_FILES)
        .build(logs_dir)
        .map_err(io::Error::other)?;
    let (writer, guard) = tracing_appender::non_blocking(appender);

    let level = if debug { "debug" } else { "info" };
    let filter = EnvFilter::try_from_env("MEHBURMC_LOG").unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "{level},launcher_core={level},mehbur_launcher={level}"
        ))
    });

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(MaskingMakeWriter(writer));
    let stderr_layer = cfg!(debug_assertions).then(|| {
        tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_writer(MaskingMakeWriter(io::stderr))
    });

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init()
        .map_err(io::Error::other)?;
    Ok(guard)
}

/// `MakeWriter` wrapper that masks secrets in every formatted event.
struct MaskingMakeWriter<M>(M);

impl<'a, M: MakeWriter<'a>> MakeWriter<'a> for MaskingMakeWriter<M> {
    type Writer = MaskingWriter<M::Writer>;
    fn make_writer(&'a self) -> Self::Writer {
        MaskingWriter {
            inner: self.0.make_writer(),
            buf: Vec::new(),
        }
    }
}

/// Buffers one event, masks it, and writes it on drop (the fmt layer creates
/// one writer per event, so masking always sees whole lines).
struct MaskingWriter<W: Write> {
    inner: W,
    buf: Vec<u8>,
}

impl<W: Write> Write for MaskingWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<W: Write> Drop for MaskingWriter<W> {
    fn drop(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let masked = mask_secrets(&String::from_utf8_lossy(&self.buf));
        let _ = self.inner.write_all(masked.as_bytes());
        let _ = self.inner.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_jwt() {
        let s = "token eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abc_DEF-123 end";
        assert_eq!(mask_secrets(s), "token *** end");
    }

    #[test]
    fn masks_cli_and_json_forms() {
        assert_eq!(
            mask_secrets("--accessToken abc123 --version 26.3"),
            "--accessToken *** --version 26.3"
        );
        assert_eq!(
            mask_secrets(r#"{"refresh_token":"secretvalue","x":1}"#),
            r#"{"refresh_token":"***","x":1}"#
        );
        assert_eq!(
            mask_secrets("access_token=xyz&foo=1"),
            "access_token=***&foo=1"
        );
    }

    #[test]
    fn masks_api_keys() {
        let key = "$2a$10$abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0";
        assert_eq!(mask_secrets(&format!("key {key} used")), "key *** used");
        assert_eq!(
            mask_secrets(r#"{"curseforgeApiKey":"anything","x":1}"#),
            r#"{"curseforgeApiKey":"***","x":1}"#
        );
        assert_eq!(mask_secrets("x-api-key: abc123"), "x-api-key: ***");
    }

    #[test]
    fn masks_friends_session() {
        assert_eq!(
            mask_secrets(r#"{"refreshToken":"q1w2e3r4t5","userId":"u"}"#),
            r#"{"refreshToken":"***","userId":"u"}"#
        );
        assert_eq!(
            mask_secrets("Authorization: Bearer abc"),
            "Authorization: ***"
        );
    }

    #[test]
    fn masks_microsoft_tokens() {
        let s = "got M.C512_BAY.0.U.-AbCdEfGhIjKlMnOpQrStUvWxYz012345 ok";
        assert_eq!(mask_secrets(s), "got *** ok");
    }

    #[test]
    fn leaves_normal_text_alone() {
        let s = "Downloading net.fabricmc:fabric-loader:0.19.5 (126151 bytes)";
        assert_eq!(mask_secrets(s), s);
    }

    #[test]
    fn writer_masks_on_drop() {
        let mut sink = Vec::new();
        {
            let mut w = MaskingWriter {
                inner: &mut sink,
                buf: Vec::new(),
            };
            w.write_all(b"accessToken: hunter2\n").unwrap();
        }
        assert_eq!(String::from_utf8(sink).unwrap(), "accessToken: ***\n");
    }
}
