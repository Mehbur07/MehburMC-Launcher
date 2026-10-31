//! Decodes an image with Windows Imaging Component, so every codec installed
//! on the machine works (HEIC, AVIF, JPEG XL, camera RAW, ...) and big photos
//! are shrunk while they are read. Windows only.
#![cfg(windows)]

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapFrameDecode, IWICBitmapSource,
    IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::core::{Interface, w};

/// Refuses absurd shapes that stay huge even after shrinking.
const MAX_OUTPUT_PIXELS: u64 = 64_000_000;

#[derive(Debug)]
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    /// Row-major RGBA8, not yet turned upright.
    pub pixels: Vec<u8>,
    /// EXIF orientation (1 = upright).
    pub orientation: u8,
}

/// Why decoding failed, for the log.
#[derive(Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl From<windows::core::Error> for Error {
    fn from(e: windows::core::Error) -> Self {
        Self(e.message())
    }
}

/// Keeps COM initialised on this thread for as long as it lives.
struct Com(bool);

impl Com {
    fn init() -> Result<Self, Error> {
        // SAFETY: no reserved pointer; balanced by `CoUninitialize` in `drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            // Already initialised another way by this thread's owner.
            return Ok(Self(false));
        }
        hr.ok()?;
        Ok(Self(true))
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: paired with the successful `CoInitializeEx` in `init`.
            unsafe { CoUninitialize() };
        }
    }
}

/// Decodes the first frame of `bytes`, shrunk so that its shorter side is at
/// most `short_side` pixels.
pub fn decode(bytes: &[u8], short_side: u32) -> Result<Decoded, Error> {
    let _com = Com::init()?;
    // SAFETY: every call follows the WIC contract. `stream` reads `bytes`,
    // which outlives all the objects here, and the COM wrappers release
    // themselves before `_com` is dropped. `pixels` is exactly
    // `stride * height` bytes long.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let stream = factory.CreateStream()?;
        stream.InitializeFromMemory(bytes)?;
        let decoder = factory.CreateDecoderFromStream(
            &stream,
            std::ptr::null(),
            WICDecodeMetadataCacheOnDemand,
        )?;
        let frame = decoder.GetFrame(0)?;
        let orientation = orientation(&frame);

        let rgba = |source: &IWICBitmapSource| -> windows::core::Result<IWICBitmapSource> {
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(
                source,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            converter.cast()
        };
        let converted = rgba(&frame.cast()?)?;

        let (mut width, mut height) = (0, 0);
        frame.GetSize(&mut width, &mut height)?;
        if width == 0 || height == 0 {
            return Err(Error("empty image".into()));
        }
        let (out_w, out_h) = shrunk(width, height, short_side);
        if u64::from(out_w) * u64::from(out_h) > MAX_OUTPUT_PIXELS {
            return Err(Error("image too large".into()));
        }
        let source = if (out_w, out_h) == (width, height) {
            converted
        } else {
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(&converted, out_w, out_h, WICBitmapInterpolationModeFant)?;
            // The scaler may hand out another pixel layout.
            rgba(&scaler.cast()?)?
        };

        let stride = out_w * 4;
        let mut pixels = vec![0; stride as usize * out_h as usize];
        source.CopyPixels(std::ptr::null(), stride, &mut pixels)?;
        Ok(Decoded {
            width: out_w,
            height: out_h,
            pixels,
            orientation,
        })
    }
}

/// The size with the shorter side at most `short_side`, keeping the shape.
fn shrunk(width: u32, height: u32, short_side: u32) -> (u32, u32) {
    let short = width.min(height);
    if short <= short_side {
        return (width, height);
    }
    let scale = |side: u32| {
        let scaled = u64::from(side) * u64::from(short_side) / u64::from(short);
        u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
    };
    (scale(width), scale(height))
}

/// 1 when the file does not say (many types have no such metadata).
fn orientation(frame: &IWICBitmapFrameDecode) -> u8 {
    // SAFETY: `value` is a valid out parameter and clears itself on drop.
    let value = unsafe {
        let Ok(reader) = frame.GetMetadataQueryReader() else {
            return 1;
        };
        let mut value = PROPVARIANT::default();
        if reader
            .GetMetadataByName(w!("System.Photo.Orientation"), &mut value)
            .is_err()
        {
            return 1;
        }
        value
    };
    match u16::try_from(&value) {
        Ok(o @ 1..=8) => o as u8,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_small_images_and_shrinks_the_short_side() {
        assert_eq!(shrunk(100, 50, 50), (100, 50));
        assert_eq!(shrunk(4000, 3000, 512), (682, 512));
        assert_eq!(shrunk(3000, 4000, 512), (512, 682));
        assert_eq!(shrunk(100_000, 1000, 1).1, 1);
    }

    #[test]
    fn unknown_data_fails() {
        assert!(decode(b"hello, this is not an image", 512).is_err());
    }
}
