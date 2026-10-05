//! Account profile photos, `launcher/avatars/<account id>.png` (ARCHITECTURE
//! K65). Without a photo the UI shows the head of the account's skin.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use base64::Engine;

use crate::error::{CoreError, Result};
use crate::paths::Paths;
use crate::skin::image::{Rgba, decode_limited, encode};

/// Largest file accepted from the picker; photos are much bigger than skins.
pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
const MAX_SIDE: u32 = 4096;
/// Stored size (square).
pub const SIZE: u32 = 256;

fn invalid(reason: impl Into<String>) -> CoreError {
    CoreError::AvatarInvalid(reason.into())
}

/// Centre square crop, resized to `size`×`size`. Shrinking averages each
/// source block (alpha-weighted, so transparent pixels do not darken edges);
/// enlarging repeats pixels.
pub fn square_downscale(img: &Rgba, size: u32) -> Rgba {
    let side = img.width.min(img.height);
    let (ox, oy) = ((img.width - side) / 2, (img.height - side) / 2);
    let span = |o: u32| {
        let a = o * side / size;
        (a, ((o + 1) * side / size).max(a + 1))
    };
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        let (y0, y1) = span(y);
        for x in 0..size {
            let (x0, x1) = span(x);
            let mut acc = [0u64; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let i = (((oy + sy) * img.width + ox + sx) * 4) as usize;
                    let a = u64::from(img.pixels[i + 3]);
                    for (sum, &v) in acc.iter_mut().zip(&img.pixels[i..i + 3]) {
                        *sum += u64::from(v) * a;
                    }
                    acc[3] += a;
                }
            }
            let n = u64::from((y1 - y0) * (x1 - x0));
            if acc[3] == 0 {
                pixels.extend([0, 0, 0, 0]);
            } else {
                let c = |v: u64| (v / acc[3]) as u8;
                pixels.extend([c(acc[0]), c(acc[1]), c(acc[2]), (acc[3] / n) as u8]);
            }
        }
    }
    Rgba {
        width: size,
        height: size,
        pixels,
    }
}

pub fn data_uri(png: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

pub struct AvatarStore {
    paths: Paths,
}

impl AvatarStore {
    pub fn new(paths: Paths) -> Self {
        Self { paths }
    }

    /// Account ids are `offline-<hex>`; anything else could leave the folder.
    fn path(&self, account_id: &str) -> Result<PathBuf> {
        let ok = !account_id.is_empty()
            && account_id.len() <= 64
            && account_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if !ok {
            return Err(CoreError::AccountNotFound(account_id.to_owned()));
        }
        Ok(self.paths.avatars().join(format!("{account_id}.png")))
    }

    /// Validates, crops and resizes `bytes`, stores the result and returns it
    /// as a `data:` URI.
    pub fn set(&self, account_id: &str, bytes: &[u8]) -> Result<String> {
        let path = self.path(account_id)?;
        let img = decode_limited(bytes, MAX_FILE_BYTES, MAX_SIDE).map_err(invalid)?;
        let png = encode(&square_downscale(&img, SIZE))?;
        crate::fsutil::write_atomic(&path, &png)?;
        Ok(data_uri(&png))
    }

    pub fn set_from_file(&self, account_id: &str, file: &Path) -> Result<String> {
        let meta = std::fs::metadata(file).map_err(|e| CoreError::io(file, e))?;
        if meta.len() > MAX_FILE_BYTES as u64 {
            return Err(invalid("file is too large"));
        }
        let bytes = std::fs::read(file).map_err(|e| CoreError::io(file, e))?;
        self.set(account_id, &bytes)
    }

    /// Back to the skin head. Missing photo is fine.
    pub fn clear(&self, account_id: &str) -> Result<()> {
        let path = self.path(account_id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CoreError::io(path, e)),
        }
    }

    /// Stored PNG of an account, if it has a photo.
    pub fn bytes(&self, account_id: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(account_id).ok()?).ok()
    }

    /// `data:` URIs of the accounts that have a photo.
    pub fn data_uris<'a>(&self, ids: impl IntoIterator<Item = &'a str>) -> HashMap<String, String> {
        ids.into_iter()
            .filter_map(|id| Some((id.to_owned(), data_uri(&self.bytes(id)?))))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin::image::decode;
    use crate::skin::image::tests::png;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> Rgba {
        Rgba {
            width: w,
            height: h,
            pixels: c.repeat((w * h) as usize),
        }
    }

    #[test]
    fn crops_the_centre_square_and_averages() {
        // 6×2: left/right columns red, middle 2×2 block half white/black.
        let mut img = solid(6, 2, [255, 0, 0, 255]);
        let set = |img: &mut Rgba, x: u32, y: u32, c: [u8; 4]| {
            let i = ((y * 6 + x) * 4) as usize;
            img.pixels[i..i + 4].copy_from_slice(&c);
        };
        set(&mut img, 2, 0, [255, 255, 255, 255]);
        set(&mut img, 2, 1, [255, 255, 255, 255]);
        set(&mut img, 3, 0, [0, 0, 0, 255]);
        set(&mut img, 3, 1, [0, 0, 0, 255]);
        let out = square_downscale(&img, 1);
        assert_eq!(out.pixels, vec![127, 127, 127, 255]);
    }

    #[test]
    fn transparent_pixels_do_not_darken_and_small_images_grow() {
        let mut img = solid(2, 1, [0, 0, 0, 0]);
        img.pixels[0..4].copy_from_slice(&[200, 100, 50, 255]);
        let out = square_downscale(&img, 4);
        assert_eq!((out.width, out.height), (4, 4));
        assert!(out.pixels.chunks(4).all(|p| p == [200, 100, 50, 255]));

        let half = square_downscale(&img, 1);
        assert_eq!(half.pixels, vec![200, 100, 50, 255]);
        let wide = Rgba {
            width: 2,
            height: 2,
            pixels: [[200, 100, 50, 255], [0, 0, 0, 0]].concat().repeat(2),
        };
        assert_eq!(square_downscale(&wide, 1).pixels, vec![200, 100, 50, 127]);
    }

    #[test]
    fn stores_resized_photo_and_clears_it() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let store = AvatarStore::new(paths.clone());

        let uri = store
            .set("offline-abc", &png(600, 400, |_, _| false))
            .unwrap();
        assert!(uri.starts_with("data:image/png;base64,"));
        let saved = decode(&store.bytes("offline-abc").unwrap()).unwrap();
        assert_eq!((saved.width, saved.height), (SIZE, SIZE));
        assert_eq!(store.data_uris(["offline-abc", "other"]).len(), 1);

        store.clear("offline-abc").unwrap();
        store.clear("offline-abc").unwrap();
        assert!(store.bytes("offline-abc").is_none());
    }

    #[test]
    fn rejects_bad_input_and_ids() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        let store = AvatarStore::new(paths);
        let e = store.set("offline-abc", b"GIF89a").unwrap_err();
        assert_eq!(e.code(), "account.avatarInvalid");
        let ok = png(8, 8, |_, _| false);
        assert!(store.set("../evil", &ok).is_err());
        assert!(store.set("", &ok).is_err());
    }
}
