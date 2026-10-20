//! Purpose: Reads the text in a picture with the OCR engine Windows ships (Windows.Media.Ocr): offline, nothing to download, and nothing stays loaded afterwards.
//! Contents: recognize_file — a picture file to text; recognize — a BGRA picture to text (lines reflowed into paragraphs, look-alike letters repaired); max_side — the largest picture side the reader takes; languages — which languages Windows can read. The languages are the ones installed in Windows (Settings, Time and language).

use std::sync::Once;

use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode, SoftwareBitmap,
};
use windows::Media::Ocr::{OcrEngine, OcrLine};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows::Win32::System::Com::CoIncrementMTAUsage;

use super::fixups::fix;
use super::reflow::{reflow, Line};
use crate::platform::Image;

/// WinRT caches the reader's factory for the life of the process, so the multithreaded apartment it was made in must outlive every call: a per-call `CoUninitialize` tears the apartment
/// down and the next read (on another thread) calls into a dead factory and crashes. Incrementing the apartment's usage once keeps it alive; every thread then joins it implicitly.
fn keep_apartment() {
    static ONCE: Once = Once::new();
    // SAFETY: a process-wide reference on the multithreaded apartment, never released.
    ONCE.call_once(|| drop(unsafe { CoIncrementMTAUsage() }));
}

const NO_LANGUAGE: &str =
    "Windows has no text recognition language installed. Add one in Settings, Time and language, Language and region.";

/// The names of the languages Windows can read text in (Settings, Time and language), for the Capture page.
pub fn languages() -> Vec<String> {
    keep_apartment();
    let Ok(list) = OcrEngine::AvailableRecognizerLanguages() else { return Vec::new() };
    list.into_iter().filter_map(|l| l.DisplayName().ok()).map(|n| n.to_string()).collect()
}

/// The largest side, in pixels, of a picture the reader accepts.
pub fn max_side() -> i32 {
    keep_apartment();
    OcrEngine::MaxImageDimension().map_or(2600, |n| n as i32)
}

/// A read line with the box around its words.
fn to_line(line: &OcrLine) -> windows::core::Result<Line> {
    let (mut left, mut top, mut bottom) = (f32::MAX, f32::MAX, f32::MIN);
    for word in line.Words()? {
        let r = word.BoundingRect()?;
        left = left.min(r.X);
        top = top.min(r.Y);
        bottom = bottom.max(r.Y + r.Height);
    }
    Ok(Line { text: line.Text()?.to_string(), left, top, bottom })
}

/// The reader for the user's languages, or a message saying how to get one.
fn engine() -> Result<OcrEngine, String> {
    keep_apartment();
    OcrEngine::TryCreateFromUserProfileLanguages().map_err(|_| NO_LANGUAGE.to_string())
}

/// The text in a bitmap: its lines reflowed into paragraphs and repaired.
fn read(engine: &OcrEngine, bitmap: &SoftwareBitmap) -> windows::core::Result<String> {
    let lines = engine.RecognizeAsync(bitmap)?.get()?.Lines()?;
    let lines: Vec<Line> = lines.into_iter().filter_map(|l| to_line(&l).ok()).filter(|l| l.top <= l.bottom).collect();
    Ok(fix(&reflow(&lines)))
}

pub fn recognize(image: &Image) -> Result<String, String> {
    let engine = engine()?;
    let limit = max_side() as u32;
    if image.w > limit || image.h > limit {
        return Err("This part of the screen is too large to read. Select a smaller area.".into());
    }
    let work = || -> windows::core::Result<String> {
        let writer = DataWriter::new()?;
        writer.WriteBytes(&image.bgra)?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &writer.DetachBuffer()?,
            BitmapPixelFormat::Bgra8,
            image.w as i32,
            image.h as i32,
            BitmapAlphaMode::Ignore,
        )?;
        read(&engine, &bitmap)
    };
    work().map_err(|e| format!("Could not read the text: {e}"))
}

/// The text in a picture file (png, jpg, bmp, gif, tiff, webp): Windows decodes it, shrinking it first when it is larger than the reader takes.
pub fn recognize_file(path: &std::path::Path) -> Result<String, String> {
    let engine = engine()?;
    let bytes = std::fs::read(path).map_err(|e| format!("Could not open the image: {e}"))?;
    let work = || -> windows::core::Result<String> {
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(&bytes)?;
        writer.StoreAsync()?.get()?;
        stream.Seek(0)?;
        let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
        let (w, h) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
        let shrink = (f64::from(max_side()) / f64::from(w.max(h).max(1))).min(1.0);
        let transform = BitmapTransform::new()?;
        transform.SetScaledWidth((f64::from(w) * shrink) as u32)?;
        transform.SetScaledHeight((f64::from(h) * shrink) as u32)?;
        let bitmap = decoder
            .GetSoftwareBitmapTransformedAsync(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Ignore,
                &transform,
                ExifOrientationMode::RespectExifOrientation,
                ColorManagementMode::DoNotColorManage,
            )?
            .get()?;
        read(&engine, &bitmap)
    };
    work().map_err(|e| format!("Could not read the image: {e}"))
}
