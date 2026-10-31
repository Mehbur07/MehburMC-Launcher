//! Account profile photos, `launcher/avatars/<account id>.png` (ARCHITECTURE
//! K65). Without a photo the UI shows the head of the account's skin.
//!
//! The picked file can be any image type (K95): Windows' own codecs first
//! (HEIC, AVIF, JPEG XL, RAW, ... when installed; they shrink big photos while
//! reading), then the `image` crate. Only this user-picked file takes this
//! path: what friends send stays PNG-only (`friends/avatar.rs`).

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use base64::Engine;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits};

use crate::error::{CoreError, Result};
use crate::paths::Paths;
use crate::skin::image::{Rgba, encode};

/// Largest file accepted from the picker; uncompressed and RAW photos are big.
pub const MAX_FILE_BYTES: usize = 200 * 1024 * 1024;
const MAX_SIDE: u32 = 30_000;
/// A 150 MP photo takes 600 MB of RGBA in the `image` crate.
const MAX_ALLOC: u64 = 700 * 1024 * 1024;
/// Windows shrinks while it reads, down to this shorter side; enough to crop
/// and average into `SIZE`.
#[cfg(windows)]
const READ_SHORT_SIDE: u32 = SIZE * 2;
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

/// The same pixels turned upright for an EXIF orientation. `img` is square,
/// so the centre crop before this is the same crop for any orientation.
fn upright(img: &Rgba, orientation: u8) -> Rgba {
    let n = img.width as usize;
    let last = n - 1;
    let mut pixels = Vec::with_capacity(img.pixels.len());
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = match orientation {
                2 => (last - x, y),
                3 => (last - x, last - y),
                4 => (x, last - y),
                5 => (y, x),
                6 => (y, last - x),
                7 => (last - y, last - x),
                8 => (last - y, x),
                _ => (x, y),
            };
            let i = (sy * n + sx) * 4;
            pixels.extend_from_slice(&img.pixels[i..i + 4]);
        }
    }
    Rgba {
        width: img.width,
        height: img.height,
        pixels,
    }
}

/// Decodes with the `image` crate; pixels and EXIF orientation. `extension`
/// names the types that carry no signature (TGA).
fn decode_with_crate(
    bytes: &[u8],
    extension: Option<&str>,
) -> std::result::Result<(Rgba, u8), ImageError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    if reader.format().is_none()
        && let Some(format) = extension.and_then(ImageFormat::from_extension)
    {
        reader.set_format(format);
    }
    reader.limits(limits);
    let mut decoder = reader.into_decoder()?;
    let orientation = decoder.orientation().map_or(1, |o| o.to_exif());
    let img = DynamicImage::from_decoder(decoder)?.into_rgba8();
    let (width, height) = img.dimensions();
    let pixels = img.into_raw();
    Ok((
        Rgba {
            width,
            height,
            pixels,
        },
        orientation,
    ))
}

/// EXIF orientation of a PNG, which Windows does not report.
#[cfg(windows)]
fn png_orientation(bytes: &[u8]) -> u8 {
    if !bytes.starts_with(b"\x89PNG") {
        return 1;
    }
    let decoder = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_decoder().ok());
    decoder
        .and_then(|mut d| d.orientation().ok())
        .map_or(1, |o| o.to_exif())
}

/// HEIC, AVIF and friends start with a `ftyp` box; they need system codecs.
fn is_iso_media(bytes: &[u8]) -> bool {
    bytes.get(4..8) == Some(b"ftyp")
}

/// Pixels and EXIF orientation of any supported image.
fn decode_photo(bytes: &[u8], extension: Option<&str>) -> Result<(Rgba, u8)> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(invalid("file is too large"));
    }
    #[cfg(windows)]
    match wic_decode::decode(bytes, READ_SHORT_SIDE) {
        Ok(d) => {
            let img = Rgba {
                width: d.width,
                height: d.height,
                pixels: d.pixels,
            };
            // HEIC and AVIF come back already turned by their `irot` box, and
            // then report 1, so a second turn cannot happen.
            let orientation = match d.orientation {
                1 => png_orientation(bytes),
                o => o,
            };
            return Ok((img, orientation));
        }
        Err(e) => tracing::debug!(?e, "Windows could not decode the photo"),
    }

    match decode_with_crate(bytes, extension) {
        Ok(decoded) => Ok(decoded),
        // Unknown to the crate as well: a missing codec or not an image.
        Err(ImageError::Unsupported(_)) => Err(CoreError::AvatarFormat),
        Err(_) if is_iso_media(bytes) => Err(CoreError::AvatarFormat),
        Err(e) => Err(invalid(e.to_string())),
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

    /// Decodes, turns upright, crops and resizes `bytes`, stores the result and
    /// returns it as a `data:` URI.
    pub fn set(&self, account_id: &str, bytes: &[u8]) -> Result<String> {
        self.store(account_id, bytes, None)
    }

    fn store(&self, account_id: &str, bytes: &[u8], extension: Option<&str>) -> Result<String> {
        let path = self.path(account_id)?;
        let (img, orientation) = decode_photo(bytes, extension)?;
        let png = encode(&upright(&square_downscale(&img, SIZE), orientation))?;
        crate::fsutil::write_atomic(&path, &png)?;
        Ok(data_uri(&png))
    }

    pub fn set_from_file(&self, account_id: &str, file: &Path) -> Result<String> {
        let meta = std::fs::metadata(file).map_err(|e| CoreError::io(file, e))?;
        if meta.len() > MAX_FILE_BYTES as u64 {
            return Err(invalid("file is too large"));
        }
        let bytes = std::fs::read(file).map_err(|e| CoreError::io(file, e))?;
        let extension = file.extension().and_then(|e| e.to_str());
        self.store(account_id, &bytes, extension)
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

    /// A square photo with four coloured corners: red, green / blue, white.
    fn corners(n: u32) -> image::RgbImage {
        image::RgbImage::from_fn(n, n, |x, y| {
            image::Rgb(match (x < n / 2, y < n / 2) {
                (true, true) => [255, 0, 0],
                (false, true) => [0, 255, 0],
                (true, false) => [0, 0, 255],
                (false, false) => [255, 255, 255],
            })
        })
    }

    /// Same colour, allowing for lossy formats.
    fn close(a: &[u8], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(&x, y)| x.abs_diff(y) < 48)
    }

    fn assert_upright(img: &Rgba) {
        let n = img.width as usize;
        let at = |x: usize, y: usize| &img.pixels[(y * n + x) * 4..(y * n + x) * 4 + 3];
        let m = n / 8;
        let got = [at(m, m), at(n - m, m), at(m, n - m), at(n - m, n - m)];
        assert!(close(got[0], [255, 0, 0]), "top left {:?}", got[0]);
        assert!(close(got[1], [0, 255, 0]), "top right {:?}", got[1]);
        assert!(close(got[2], [0, 0, 255]), "bottom left {:?}", got[2]);
        assert!(close(got[3], [255, 255, 255]), "bottom right {:?}", got[3]);
    }

    fn encoded(format: ImageFormat) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        let rgb = DynamicImage::ImageRgb8(corners(64));
        // ICO only takes RGBA8, Farbfeld only 16 bits.
        let img = match format {
            ImageFormat::Ico => DynamicImage::ImageRgba8(rgb.into_rgba8()),
            ImageFormat::Farbfeld => DynamicImage::ImageRgba16(rgb.into_rgba16()),
            _ => rgb,
        };
        img.write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    fn temp_store() -> (tempfile::TempDir, AvatarStore) {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::at(tmp.path().join("MehburMC"));
        paths.ensure_layout().unwrap();
        (tmp, AvatarStore::new(paths))
    }

    #[test]
    fn stores_every_common_type() {
        use ImageFormat as F;
        let (tmp, store) = temp_store();
        for format in [
            F::Png,
            F::Jpeg,
            F::Gif,
            F::WebP,
            F::Bmp,
            F::Tiff,
            F::Ico,
            F::Tga,
            F::Qoi,
            F::Pnm,
            F::Farbfeld,
        ] {
            let bytes = encoded(format);
            let extension = format.extensions_str()[0];
            let (img, _) = decode_with_crate(&bytes, Some(extension))
                .unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_upright(&square_downscale(&img, SIZE));

            let file = tmp.path().join(format!("photo.{extension}"));
            std::fs::write(&file, &bytes).unwrap();
            store.set_from_file("offline-abc", &file).unwrap();
            let saved = decode(&store.bytes("offline-abc").unwrap()).unwrap();
            assert_eq!((saved.width, saved.height), (SIZE, SIZE), "{format:?}");
            assert_upright(&saved);
        }
    }

    /// A JPEG of `img` saying it must be shown with EXIF `orientation`.
    fn jpeg_with_orientation(img: &image::RgbImage, orientation: u8) -> Vec<u8> {
        let mut plain = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img.clone())
            .write_to(&mut plain, ImageFormat::Jpeg)
            .unwrap();
        let plain = plain.into_inner();
        let mut exif = b"Exif\0\0II*\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        exif.extend([orientation, 0, 0, 0, 0, 0, 0, 0]);
        let mut out = plain[..2].to_vec();
        out.extend([0xff, 0xe1]);
        out.extend(u16::try_from(exif.len() + 2).unwrap().to_be_bytes());
        out.extend(exif);
        out.extend(&plain[2..]);
        out
    }

    #[test]
    fn turns_photos_upright() {
        use image::imageops as ops;
        let up = corners(64);
        // What a camera stores for each orientation, so that showing it
        // with that orientation gives `up`.
        let stored = [
            (1, up.clone()),
            (2, ops::flip_horizontal(&up)),
            (3, ops::rotate180(&up)),
            (4, ops::flip_vertical(&up)),
            (5, ops::flip_horizontal(&ops::rotate90(&up))),
            (6, ops::rotate270(&up)),
            (7, ops::flip_horizontal(&ops::rotate270(&up))),
            (8, ops::rotate90(&up)),
        ];
        let (_t, store) = temp_store();
        for (orientation, img) in stored {
            let bytes = jpeg_with_orientation(&img, orientation);
            let (raw, read) = decode_with_crate(&bytes, None).unwrap();
            assert_eq!(read, orientation);
            assert_upright(&upright(&square_downscale(&raw, SIZE), read));

            store.set("offline-abc", &bytes).unwrap();
            assert_upright(&decode(&store.bytes("offline-abc").unwrap()).unwrap());
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_reads_orientation_and_shrinks() {
        let big = corners(1200);
        let bytes = jpeg_with_orientation(&image::imageops::rotate270(&big), 6);
        let d = wic_decode::decode(&bytes, READ_SHORT_SIDE).unwrap();
        assert_eq!(d.orientation, 6);
        assert_eq!((d.width, d.height), (READ_SHORT_SIDE, READ_SHORT_SIDE));
        // Colours survive the shrink (the scaler changes the pixel layout).
        let img = Rgba {
            width: d.width,
            height: d.height,
            pixels: d.pixels,
        };
        assert_upright(&upright(&img, d.orientation));
    }

    /// Real photo files: `MEHBUR_PHOTOS=<dir> cargo test -p launcher-core --lib
    /// real_photos -- --ignored --nocapture`. Each file shows the corner
    /// pattern of `corners` when displayed correctly.
    #[test]
    #[ignore = "needs sample files"]
    fn real_photos() {
        let dir = std::env::var("MEHBUR_PHOTOS").expect("MEHBUR_PHOTOS");
        let (_t, store) = temp_store();
        let mut failed = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let result = store.set_from_file("offline-abc", &path).map(|_| {
                let saved = decode(&store.bytes("offline-abc").unwrap()).unwrap();
                std::panic::catch_unwind(|| assert_upright(&saved))
            });
            match result {
                Ok(Ok(())) => println!("OK        {name}"),
                Ok(Err(_)) => {
                    failed += 1;
                    println!("SIDEWAYS  {name}");
                }
                Err(e) => {
                    failed += 1;
                    println!("FAILED    {name}: {e}");
                }
            }
        }
        assert_eq!(failed, 0);
    }

    #[test]
    fn says_so_when_nothing_can_open_a_file() {
        let (_t, store) = temp_store();
        let e = store
            .set("offline-abc", b"hello, not an image")
            .unwrap_err();
        assert_eq!(e.code(), "account.avatarFormat");
        // A HEIC header without a decodable picture.
        let e = store
            .set("offline-abc", b"\0\0\0\x18ftypheic\0\0\0\0mif1heic")
            .unwrap_err();
        assert_eq!(e.code(), "account.avatarFormat");
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
