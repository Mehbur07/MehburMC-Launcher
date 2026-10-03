//! PNG validation for skins and capes, plus slim-arm detection.
//!
//! Images are fully decoded (not just sniffed) so a corrupt or disguised file
//! never reaches the library, the game or the 3D preview.

use std::io::Cursor;

use super::SkinModel;
use crate::error::{CoreError, Result};

/// Upper bound for an imported file; real skins are a few KiB.
pub const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;
/// HD skins up to 1024×1024 (CustomSkinLoader and skinview3d support them).
const MAX_SIDE: u32 = 1024;

#[derive(Debug)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8.
    pub pixels: Vec<u8>,
}

impl Rgba {
    fn alpha(&self, x: u32, y: u32) -> u8 {
        self.pixels[((y * self.width + x) * 4 + 3) as usize]
    }

    fn is_opaque_black(&self, x: u32, y: u32) -> bool {
        let i = ((y * self.width + x) * 4) as usize;
        self.pixels[i..i + 4] == [0, 0, 0, 255]
    }
}

fn invalid(reason: &str) -> CoreError {
    CoreError::SkinInvalid(reason.to_owned())
}

/// Decodes any PNG colour type into RGBA8.
pub fn decode(bytes: &[u8]) -> Result<Rgba> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(invalid("file is too large"));
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid("not a PNG image"));
    }
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: 16 * 1024 * 1024,
        },
    );
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| invalid(&e.to_string()))?;
    let (w, h) = (reader.info().width, reader.info().height);
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(invalid("unsupported size"));
    }
    let mut buf = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| invalid("image too large"))?
    ];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| invalid(&e.to_string()))?;
    buf.truncate(frame.buffer_size());

    use png::ColorType as C;
    let pixels: Vec<u8> = match frame.color_type {
        C::Rgba => buf,
        C::Rgb => buf
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|&[r, g, b]| [r, g, b, 255])
            .collect(),
        C::GrayscaleAlpha => buf
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|&[g, a]| [g, g, g, a])
            .collect(),
        C::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        C::Indexed => return Err(invalid("unexpanded palette")),
    };
    if pixels.len() != (w * h * 4) as usize {
        return Err(invalid("unexpected pixel layout"));
    }
    Ok(Rgba {
        width: w,
        height: h,
        pixels,
    })
}

/// 64×64 (1.8+), legacy 64×32, and HD multiples of both.
pub fn check_skin(img: &Rgba) -> Result<()> {
    let (w, h) = (img.width, img.height);
    let ok = w >= 64 && w.is_power_of_two() && (h == w || h * 2 == w);
    if ok {
        Ok(())
    } else {
        Err(CoreError::SkinInvalid(format!(
            "skin must be 64×64 or 64×32 (or an HD multiple), got {w}×{h}"
        )))
    }
}

/// Mojang 64×32 (and HD multiples) or the old 22×17 layout (and multiples).
pub fn check_cape(img: &Rgba) -> Result<()> {
    let (w, h) = (img.width, img.height);
    let modern = w >= 64 && w % 64 == 0 && h * 2 == w;
    let old = w % 22 == 0 && w * 17 == h * 22;
    if modern || old {
        Ok(())
    } else {
        Err(CoreError::SkinInvalid(format!(
            "cape must be 64×32 or 22×17 (or an HD multiple), got {w}×{h}"
        )))
    }
}

/// Slim ("Alex") skins leave the outer 1px column of each arm empty; tools
/// paint it either transparent or opaque black. Legacy 64×32 skins predate
/// slim arms.
pub fn detect_model(img: &Rgba) -> SkinModel {
    if img.height != img.width {
        return SkinModel::Classic;
    }
    let s = img.width / 64;
    // Right arm: top/bottom faces (50..52, 16..20) and the side (54..56, 20..32).
    let areas = [(50, 16, 2, 4), (54, 20, 2, 12)];
    let mut transparent = true;
    let mut black = true;
    for (x0, y0, aw, ah) in areas {
        for y in y0 * s..(y0 + ah) * s {
            for x in x0 * s..(x0 + aw) * s {
                transparent &= img.alpha(x, y) == 0;
                black &= img.is_opaque_black(x, y);
            }
        }
    }
    if transparent || black {
        SkinModel::Slim
    } else {
        SkinModel::Classic
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Encodes an RGBA image, painting every pixel opaque grey except where
    /// `hole` returns true (fully transparent).
    pub fn png(w: u32, h: u32, hole: impl Fn(u32, u32) -> bool) -> Vec<u8> {
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                if hole(x, y) {
                    data.extend([0, 0, 0, 0]);
                } else {
                    data.extend([120, 90, 60, 255]);
                }
            }
        }
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&data).unwrap();
        out
    }

    fn slim_hole(s: u32) -> impl Fn(u32, u32) -> bool {
        move |x, y| {
            (50 * s..52 * s).contains(&x) && (16 * s..20 * s).contains(&y)
                || (54 * s..56 * s).contains(&x) && (20 * s..32 * s).contains(&y)
        }
    }

    #[test]
    fn validates_sizes() {
        for (w, h) in [(64, 64), (64, 32), (128, 128), (512, 256)] {
            check_skin(&decode(&png(w, h, |_, _| false)).unwrap()).unwrap();
        }
        for (w, h) in [(64, 48), (32, 32), (96, 96)] {
            let e = check_skin(&decode(&png(w, h, |_, _| false)).unwrap()).unwrap_err();
            assert_eq!(e.code(), "skin.invalid");
        }
        for (w, h) in [(64, 32), (128, 64), (22, 17), (44, 34)] {
            check_cape(&decode(&png(w, h, |_, _| false)).unwrap()).unwrap();
        }
        assert!(check_cape(&decode(&png(64, 64, |_, _| false)).unwrap()).is_err());
    }

    #[test]
    fn rejects_non_png_and_garbage() {
        assert_eq!(decode(b"GIF89a....").unwrap_err().code(), "skin.invalid");
        let mut broken = png(64, 64, |_, _| false);
        broken.truncate(60);
        assert!(decode(&broken).is_err());
        assert!(decode(&vec![0u8; MAX_FILE_BYTES + 1]).is_err());
    }

    #[test]
    fn detects_slim_arms() {
        let classic = decode(&png(64, 64, |_, _| false)).unwrap();
        assert_eq!(detect_model(&classic), SkinModel::Classic);
        let slim = decode(&png(64, 64, slim_hole(1))).unwrap();
        assert_eq!(detect_model(&slim), SkinModel::Slim);
        let hd = decode(&png(128, 128, slim_hole(2))).unwrap();
        assert_eq!(detect_model(&hd), SkinModel::Slim);
        let legacy = decode(&png(64, 32, |_, _| true)).unwrap();
        assert_eq!(detect_model(&legacy), SkinModel::Classic);
    }

    #[test]
    fn converts_rgb_to_rgba() {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, 64, 32);
        enc.set_color(png::ColorType::Rgb);
        enc.write_header()
            .unwrap()
            .write_image_data(&vec![7u8; 64 * 32 * 3])
            .unwrap();
        let img = decode(&out).unwrap();
        assert_eq!(&img.pixels[..4], &[7, 7, 7, 255]);
    }
}
