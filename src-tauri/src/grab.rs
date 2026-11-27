//! File: grab.rs
//! Purpose: The Grab Text shortcut: the user drags over any part of the screen, Cue reads the text in it and offers it in the palette, where Enter copies it (or the result of an action on it).
//! Contents: cancel — Escape closes the region selector from the keyboard hook; is_busy — a grab is in progress; on_press — shortcut pressed, run the flow on its own thread (one at a time, and never while an earlier read is still alive); run — pick the region, read it, open the palette in copy mode; read_with_limit — read on a worker thread; answer — wait for it with a time limit, cancel it on time-out and give it a grace to answer; rows — the recognised text as the first row, its repaired version as a second; notice — a line on the pill; list_ocr_languages — the languages the reader knows, for the Capture page.
//! Nothing is kept: the screen picture lives on this thread for the length of the grab and is freed before the palette opens. A read that does not stop is cancelled; while its worker is still alive no new grab starts (and no new worker), and the user is told to restart Cue. A result that lands in the grace after the limit is shown; one that lands later is dropped.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::actions::Source;
use crate::palette::{self, Group, Item};
use crate::platform::{self, overlay, Reading, Stop};
use crate::prefs::Prefs;
use crate::{log, preview, sanitize, status};

/// Escape: closes the region selector if it is up, whichever window has the keyboard.
pub fn cancel() {
    platform::close_selector();
}

/// A grab is running (until the palette is handed to the preview window; an open palette is covered by `busy`): a second press meanwhile is ignored.
static BUSY: AtomicBool = AtomicBool::new(false);
/// Reader threads that have not exited yet. A read that was cancelled but still runs keeps this above zero, which keeps new grabs out: another worker would hold another screenshot.
static WORKERS: AtomicUsize = AtomicUsize::new(0);

/// How long the text reader may take: a stuck read must not hold Grab Text for ever.
const READ_LIMIT: Duration = Duration::from_secs(30);
/// How long a cancelled read gets to exit before it counts as stuck.
const EXIT_WITHIN: Duration = Duration::from_secs(5);

/// A grab is in progress.
pub fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// The shortcut was pressed: a grab starts unless one runs, or an earlier read has not exited (then the user is told once). Once the screen picture is freed its pages are given back. A dictionary card that is up gives way.
pub fn on_press(app: &AppHandle) {
    crate::dictionary::cancel();
    if crate::busy::busy(app) || BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    if WORKERS.load(Ordering::Acquire) > 0 {
        BUSY.store(false, Ordering::Release);
        return status::report(app, "Grab Text is stuck reading an earlier selection. Restart Cue to use it again.");
    }
    let handle = app.clone();
    let started = crate::guard::spawn_or_report(app, "grab", 1024 * 1024, "Grab Text", move || {
        let _release = crate::guard::Release(&BUSY);
        run(&handle);
        platform::trim_working_set();
    });
    if !started {
        BUSY.store(false, Ordering::Release);
    }
}

/// The languages Windows can read text in, for the Capture page; the lookup is quick but goes through WinRT, so it runs off the main thread.
#[tauri::command]
pub async fn list_ocr_languages() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(platform::ocr_languages).await.unwrap_or_default()
}

/// Shows a line on the pill.
fn notice(text: &str) {
    let prefs = Prefs::load();
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// Counts a reader thread while it lives; dropping it (on any way out, a panic included) lets new grabs in again.
struct Alive;

impl Alive {
    /// Registers one more reader thread.
    fn new() -> Alive {
        WORKERS.fetch_add(1, Ordering::AcqRel);
        Alive
    }
}

impl Drop for Alive {
    /// The reader thread is gone.
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Reads the picture on its own thread, which frees the picture before it reports. Input: the picture. Returns the reading or the error to show (see `answer`).
fn read_with_limit(image: platform::Image) -> Result<Reading, String> {
    let (tx, rx) = mpsc::channel();
    let stop = Arc::new(Stop::new());
    let (worker_stop, alive) = (stop.clone(), Alive::new());
    crate::guard::spawn("grab-read", 1024 * 1024, move || {
        let result = platform::recognize_text(&image, &worker_stop);
        drop(image);
        drop(alive);
        let _ = tx.send(result);
    })
    .map_err(|e| format!("Could not start reading the text: {e}"))?;
    answer(&rx, &stop, READ_LIMIT, EXIT_WITHIN)
}

/// Waits for the reader's answer. On `limit` the read is cancelled and given `grace` to answer: an answer in the grace is used as it is (a finished read is shown, a stopped one says it took too long); if none comes, the worker stays counted (no new grab starts) and the answer says to restart Cue. Inputs: the reader's channel, its stop, the limit and the grace. Returns the reading or the error to show.
fn answer(rx: &mpsc::Receiver<Result<Reading, String>>, stop: &Stop, limit: Duration, grace: Duration) -> Result<Reading, String> {
    match rx.recv_timeout(limit) {
        Ok(result) => return result,
        Err(mpsc::RecvTimeoutError::Disconnected) => return Err("Reading the text failed.".into()),
        Err(mpsc::RecvTimeoutError::Timeout) => stop.cancel(),
    }
    match rx.recv_timeout(grace) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err("Grab Text is stuck. Restart Cue to use it again.".into()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err("Reading the text took too long and was stopped.".into()),
    }
}

/// One grab: pick the region, read it, open the palette. The reader's lines that could not be read are reported, never dropped silently. On a failed read the hidden palette is dismissed, since it would otherwise block every shortcut that needs it. The palette's webview loads while the text is read.
fn run(app: &AppHandle) {
    let program = platform::foreground_app();
    let image = match platform::grab_region(&Prefs::load().theme()) {
        Ok(Some(image)) => image,
        Ok(None) => return, // cancelled
        Err(e) => return status::report(app, &e),
    };
    preview::warm(app);
    let (size, started) = ((image.w, image.h), Instant::now());
    let reading = match read_with_limit(image) {
        Ok(reading) => reading,
        Err(e) => {
            preview::dismiss(app);
            return status::report(app, &e);
        }
    };
    let (raw, repaired) = (sanitize::clean_selection(&reading.raw), sanitize::clean_selection(&reading.repaired));
    log::note(&format!("grab: read {}x{} px in {} ms, {} characters", size.0, size.1, started.elapsed().as_millis(), raw.chars().count()));
    if reading.skipped > 0 {
        status::report(app, &format!("{} line(s) of the picture could not be read and are missing from the text.", reading.skipped));
    }
    if raw.is_empty() {
        preview::dismiss(app);
        return notice("No text found");
    }
    let mut items = rows(&raw, &repaired);
    items.extend(palette::build(&raw, &program, None, Source::Capture));
    preview::open_palette_copy(app, (raw, program), items);
}

/// The first rows of the palette: the text as recognised, and (only when it differs) the repaired version as a separate choice, so a repair that is wrong costs nothing.
fn rows(raw: &str, repaired: &str) -> Vec<Item> {
    let row = |id: &str, name: &str, hint: &str, text: &str| Item {
        id: id.into(),
        name: name.into(),
        hint: hint.into(),
        group: Group::Captured,
        model: false,
        diff: false,
        result: Some(text.to_string()),
    };
    let mut rows = vec![row("grab:text", "Recognized Text", "The text as read from the screen", raw)];
    if !repaired.is_empty() && repaired != raw {
        rows.push(row(
            "grab:repaired",
            "Repaired Text",
            "Look-alike letters and line-end hyphens repaired, a guess that can be wrong",
            repaired,
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{answer, rows, Alive, Reading, Stop, WORKERS};

    /// A reading with `raw` as its text.
    fn reading(raw: &str) -> Result<Reading, String> {
        Ok(Reading { raw: raw.into(), repaired: String::new(), skipped: 0 })
    }

    /// A result that lands after the limit but within the grace is shown, not replaced by a "took too long" error.
    #[test]
    fn a_result_in_the_grace_is_shown() {
        let (tx, rx) = mpsc::channel();
        let stop = Stop::new();
        let late = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(60));
            tx.send(reading("late")).unwrap();
        });
        let got = answer(&rx, &stop, Duration::from_millis(20), Duration::from_secs(5));
        late.join().unwrap();
        assert_eq!(got.map(|r| r.raw), Ok("late".into()));
    }

    /// A read that never answers, not even in the grace, tells the user to restart Cue.
    #[test]
    fn a_read_that_never_answers_is_stuck() {
        let (_tx, rx) = mpsc::channel::<Result<Reading, String>>();
        let got = answer(&rx, &Stop::new(), Duration::from_millis(5), Duration::from_millis(5));
        assert_eq!(got.err().as_deref(), Some("Grab Text is stuck. Restart Cue to use it again."));
    }

    /// A reader thread is counted while it lives, on every way out, so a grab cannot start while an earlier read still holds its picture.
    #[test]
    fn a_reader_thread_is_counted_until_it_is_gone() {
        let before = WORKERS.load(Ordering::Acquire);
        let alive = Alive::new();
        assert_eq!(WORKERS.load(Ordering::Acquire), before + 1);
        drop(alive);
        assert_eq!(WORKERS.load(Ordering::Acquire), before);
    }

    /// The recognised text is always the first row; a repaired version is a second row only when it differs.
    #[test]
    fn a_repaired_row_is_offered_only_when_it_differs() {
        assert_eq!(rows("HCl", "HCl").len(), 1);
        let both = rows("HCl", "HCI");
        assert_eq!((both.len(), both[0].result.as_deref(), both[1].result.as_deref()), (2, Some("HCl"), Some("HCI")));
        assert_eq!(rows("x", "").len(), 1);
    }
}
