//! Purpose: Inter for the native windows, registered for this process only while a window that draws text is on screen.
//! Contents: Private — guard that adds the bundled Inter files (Regular, Medium, SemiBold, Italic) to GDI (AddFontMemResourceEx) and removes them on drop; create — a GDI font in Inter, or in Segoe UI Variable when registering failed; face_for — the face and weight GDI is asked for.

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Gdi::*;

const FILES: [&[u8]; 4] = [
    include_bytes!("../../../assets/fonts/Inter-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Medium.ttf"),
    include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/Inter-Italic.ttf"),
];
const FALLBACK: PCWSTR = w!("Segoe UI Variable Text");

/// Holds the registered fonts. A window owns one from the moment it opens until it closes, so nothing stays loaded while idle.
pub struct Private(Vec<HANDLE>);

impl Private {
    /// Registers the four files; any that fails is logged and, if none work, `create` uses the fallback face.
    pub fn load() -> Private {
        let mut handles = Vec::new();
        for file in FILES {
            let mut count = 0u32;
            // SAFETY: `file` is a 'static buffer; GDI copies it before returning.
            let handle = unsafe { AddFontMemResourceEx(file.as_ptr().cast(), file.len() as u32, None, std::ptr::addr_of_mut!(count)) };
            if handle.is_invalid() || count == 0 {
                crate::log::note("fonts: Inter could not be registered, using Segoe UI Variable");
                continue;
            }
            handles.push(handle);
        }
        // Mixed Inter weights next to a fallback look worse than one face: all or nothing.
        if handles.len() < FILES.len() {
            drop(Private(handles));
            return Private(Vec::new());
        }
        Private(handles)
    }

    /// A font `dip` tall at `scale` in the weight 400, 500 or 600.
    pub unsafe fn create(&self, dip: f32, scale: f32, weight: i32) -> HFONT {
        self.make(dip, scale, weight, false)
    }

    /// The italic of the regular weight (Inter's own italic, not a slanted copy).
    pub unsafe fn create_italic(&self, dip: f32, scale: f32) -> HFONT {
        self.make(dip, scale, 400, true)
    }

    unsafe fn make(&self, dip: f32, scale: f32, weight: i32, italic: bool) -> HFONT {
        let (face, weight) = face_for(!self.0.is_empty(), weight);
        CreateFontW(
            -((dip * scale).round() as i32),
            0,
            0,
            0,
            weight,
            italic as u32,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            face,
        )
    }
}

impl Drop for Private {
    fn drop(&mut self) {
        // SAFETY: each handle came from AddFontMemResourceEx and is removed once.
        self.0.iter().for_each(|h| unsafe {
            let _ = RemoveFontMemResourceEx(*h);
        });
    }
}

/// GDI names a static Inter file by its family name field, so the Medium and SemiBold files are their own families at weight 400.
fn face_for(loaded: bool, weight: i32) -> (PCWSTR, i32) {
    match (loaded, weight) {
        (false, _) => (FALLBACK, weight),
        (true, 500) => (w!("Inter Medium"), 400),
        (true, 600) => (w!("Inter SemiBold"), 400),
        (true, _) => (w!("Inter"), 400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inter_weights_are_separate_families_and_the_fallback_keeps_its_weight() {
        assert_eq!(face_for(true, 600).1, 400);
        assert_eq!(face_for(false, 600).1, 600);
        assert_ne!(face_for(true, 500).0, face_for(true, 400).0);
    }
}
