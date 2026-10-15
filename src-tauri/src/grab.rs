//! Purpose: The Grab Text shortcut: the user drags over any part of the screen, Cue reads the text in it and offers it in the palette, where Enter copies it (or the result of an action on it).
//! Contents: on_press — shortcut pressed, run the flow on its own thread (one at a time); run — pick the region, read it, open the palette in copy mode.
//! Nothing is kept: the screen picture lives on this thread for the length of the grab and is freed before the palette opens.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::actions::Source;
use crate::palette::{self, Group, Item};
use crate::platform::{self, overlay};
use crate::prefs::Prefs;
use crate::{log, preview, sanitize, status};

/// A grab is running (or its palette is open): a second press meanwhile is ignored.
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn on_press(app: &AppHandle) {
    if crate::dictation::is_recording() || preview::is_open(app) || BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let spawned = crate::guard::spawn("grab", 1024 * 1024, move || {
        let _release = crate::guard::Release(&BUSY);
        run(&app);
        platform::trim_working_set(); // the screen picture is freed; give the pages back
    });
    if let Err(e) = spawned {
        BUSY.store(false, Ordering::Release);
        eprintln!("cue: could not start the grab flow: {e}");
    }
}

/// The languages Windows can read text in, for the Capture page; the lookup is quick but goes through WinRT, so it runs off the main thread.
#[tauri::command]
pub async fn list_ocr_languages() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(platform::ocr_languages).await.unwrap_or_default()
}

fn notice(app: &AppHandle, text: &str) {
    let prefs = Prefs::load(app);
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// How long the text reader may take: it has no time limit of its own, and a stuck read must not leave Grab Text busy for ever.
const READ_LIMIT: Duration = Duration::from_secs(30);

/// Reads the picture on its own thread; the picture is freed there when it is done.
fn read_with_limit(image: platform::Image) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    crate::guard::spawn("grab-read", 1024 * 1024, move || {
        let _ = tx.send(platform::recognize_text(&image));
    })
    .map_err(|e| format!("Could not start reading the text: {e}"))?;
    rx.recv_timeout(READ_LIMIT).map_err(|_| "Reading the text took too long and was stopped.".to_string())?
}

fn run(app: &AppHandle) {
    let program = platform::foreground_app();
    let image = match platform::grab_region() {
        Ok(Some(image)) => image,
        Ok(None) => return, // cancelled
        Err(e) => return status::report(app, &e),
    };
    preview::warm(app); // the palette's webview loads while the text is read
    let (size, started) = ((image.w, image.h), Instant::now());
    let text = match read_with_limit(image) {
        Ok(text) => sanitize::clean_selection(&text),
        Err(e) => {
            preview::dismiss(app); // the hidden window would otherwise block every shortcut that needs it
            return status::report(app, &e);
        }
    };
    log::note(&format!("grab: read {}x{} px in {} ms, {} characters", size.0, size.1, started.elapsed().as_millis(), text.chars().count()));
    if text.is_empty() {
        preview::dismiss(app);
        return notice(app, "No text found");
    }
    let mut items = vec![Item {
        id: "grab:text".into(),
        name: "Recognized Text".into(),
        hint: "The text as read from the screen".into(),
        group: Group::Captured,
        model: false,
        result: Some(text.clone()),
    }];
    items.extend(palette::build(&text, &program, None, Source::Capture));
    preview::open_palette_copy(app, (text, program), items);
}
