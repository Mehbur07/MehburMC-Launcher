//! Project icons for the browser. The webview may not load remote images
//! (CSP), so icons are fetched here (allowlisted hosts only), cached in
//! `cache/icons/` and handed over as `data:` URIs.

use base64::Engine;

use crate::ctx::Ctx;
use crate::error::{CoreError, Result};
use crate::fsutil::write_atomic;

const MAX_ICON_BYTES: usize = 1024 * 1024;

/// Raster formats only; SVG is refused.
fn mime(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        [
            b'R',
            b'I',
            b'F',
            b'F',
            _,
            _,
            _,
            _,
            b'W',
            b'E',
            b'B',
            b'P',
            ..,
        ] => Some("image/webp"),
        _ => None,
    }
}

fn to_uri(bytes: &[u8]) -> Result<String> {
    let m = mime(bytes).ok_or_else(|| CoreError::InvalidSetting("icon format".into()))?;
    Ok(format!(
        "data:{m};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

pub async fn data_uri(ctx: &Ctx, url: &str) -> Result<String> {
    use sha1::Digest;
    ctx.http.allowlist().check(url)?;
    let key = hex::encode(sha1::Sha1::digest(url.as_bytes()));
    let file = ctx.paths.cache().join("icons").join(key);
    if let Ok(bytes) = std::fs::read(&file) {
        return to_uri(&bytes);
    }
    let bytes = ctx.http.get_bytes(url).await?;
    if bytes.len() > MAX_ICON_BYTES {
        return Err(CoreError::InvalidSetting("icon size".into()));
    }
    let uri = to_uri(&bytes)?;
    write_atomic(&file, &bytes)?;
    Ok(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_formats() {
        assert_eq!(mime(&[0x89, b'P', b'N', b'G', 0]), Some("image/png"));
        assert_eq!(mime(b"RIFF1234WEBPVP8"), Some("image/webp"));
        assert_eq!(mime(b"<svg onload=alert(1)>"), None);
        assert!(
            to_uri(&[0x89, b'P', b'N', b'G', 1])
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
    }
}
