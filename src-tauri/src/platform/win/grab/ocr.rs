//! File: ocr.rs
//! Purpose: Reads the text in a picture with the OCR engine Windows ships (Windows.Media.Ocr): offline, nothing to download, and nothing stays loaded afterwards.
//! Contents: recognize_file — a picture file to text (repaired); recognize — a BGRA picture to a `Reading` (the raw text as recognised, a separate repaired text, and the lines that could not be read), cancellable through a `Stop`; max_side — the largest picture side the reader takes; languages — which languages Windows can read. The languages are the ones installed in Windows (Settings, Time and language); keep_apartment / engine / read / to_line — the reader and its steps.

use std::sync::Once;

use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode, ExifOrientationMode, SoftwareBitmap,
};
use windows::Media::Ocr::{OcrEngine, OcrLine};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows::Win32::System::Com::CoIncrementMTAUsage;

use super::fixups::fix;
use super::reflow::{reflow, Line};
use crate::platform::{Image, Reading, Stop};

/// WinRT caches the reader's factory for the life of the process, so the multithreaded apartment it was made in must outlive every call: a per-call `CoUninitialize` tears the apartment
/// down and the next read (on another thread) calls into a dead factory and crashes. Incrementing the apartment's usage once keeps it alive; every thread then joins it implicitly.
/// Safety: takes a process-wide reference on the multithreaded apartment, never released.
fn keep_apartment() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| drop(unsafe { CoIncrementMTAUsage() }));
}

/// What the user is told when a read was stopped before it finished.
const STOPPED: &str = "Reading the text took too long and was stopped.";

/// What the user is told when Windows has no recognition language.
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

/// A cancel call that may run on another thread. WinRT async operations are free-threaded and `Cancel` may be called from any thread, so moving the call to the thread that asks for the stop is sound.
struct Agile<F>(F);

// Safety: see `Agile`; the closure only calls `Cancel` on an agile operation.
unsafe impl<F> Send for Agile<F> {}

impl<F: FnOnce()> Agile<F> {
    /// Makes the call; a finished operation ignores it.
    fn run(self) {
        (self.0)()
    }
}

/// The text in a bitmap: its lines reflowed into paragraphs. The raw text keeps every character the reader gave and every screen hyphen; the repaired text joins hyphenated words and repairs look-alike letters (a guess, so it is a separate choice). Lines whose words could not be read are counted, never silently dropped. The reader's operation is cancelled when `stop` is asked.
fn read(engine: &OcrEngine, bitmap: &SoftwareBitmap, stop: &Stop) -> windows::core::Result<Reading> {
    let operation = engine.RecognizeAsync(bitmap)?;
    let pending = operation.clone();
    let cancel = Agile(move || drop(pending.Cancel()));
    stop.arm(move || cancel.run());
    let found = operation.get()?.Lines()?;
    let (mut lines, mut skipped) = (Vec::new(), 0);
    for line in found {
        match to_line(&line).ok().filter(|l| l.top <= l.bottom) {
            Some(line) => lines.push(line),
            None => skipped += 1,
        }
    }
    Ok(Reading { raw: reflow(&lines, false), repaired: fix(&reflow(&lines, true)), skipped })
}

/// The text in a BGRA picture; Err with a line for the user when the picture is larger than the reader takes, no language is installed, the read was stopped, or reading fails.
pub fn recognize(image: &Image, stop: &Stop) -> Result<Reading, String> {
    let engine = engine()?;
    let limit = max_side() as u32;
    if image.w > limit || image.h > limit {
        return Err("This part of the screen is too large to read. Select a smaller area.".into());
    }
    if stop.stopped() {
        return Err(STOPPED.into());
    }
    let work = || -> windows::core::Result<Reading> {
        let writer = DataWriter::new()?;
        writer.WriteBytes(&image.bgra)?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &writer.DetachBuffer()?,
            BitmapPixelFormat::Bgra8,
            image.w as i32,
            image.h as i32,
            BitmapAlphaMode::Ignore,
        )?;
        read(&engine, &bitmap, stop)
    };
    work().map_err(|e| if stop.stopped() { STOPPED.to_string() } else { format!("Could not read the text: {e}") })
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
        read(&engine, &bitmap, &Stop::new()).map(|reading| reading.repaired)
    };
    work().map_err(|e| format!("Could not read the image: {e}"))
}
