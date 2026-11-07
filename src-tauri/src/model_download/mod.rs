//! Purpose: Download models (speech, text, dictionary packs) side by side: every start runs at once on its own short-lived thread; nothing queues and nothing is capped.
//! Contents: Wanted / Shared / After — what a model needs; start / cancel / reserve_delete / view / shutdown — the interface the commands use (`state` keeps the books, `worker` runs a download,
//! `transfer` fetches one file, `http` talks HTTPS); recover — tidy half-finished installs at launch.
//! Events: `model-progress {id, generation, done, total}`, `model-finished {id, generation, revision, outcome, error}`, `model-changed {revision}` (a download started or a delete took place).
//! shortcut: Quit waits up to 3 s for downloads to stop and does not kill an unpacking `tar`, which only writes into a staging folder that the next launch deletes; upgrade: keep the child in the job and kill it on cancel.
//! shortcut: all models live on one drive (the app data folder), so disk room is one number; split it per drive if models ever get a user-chosen folder.

mod http;
mod state;
mod transfer;
mod worker;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub use state::{DeleteGuard, View};

use crate::speech::catalog::ModelFile;
use crate::status;
use state::{Manager, Sizes};
use worker::Outcome;

static MANAGER: Manager = Manager::new();

/// What runs once every file is in place (for example unpacking an archive).
pub type After = Box<dyn FnOnce(&AppHandle) -> Result<(), String> + Send>;

/// A step run while the lock of a shared file is held, right after that file arrived.
pub type Step = fn(&AppHandle) -> Result<(), String>;

/// How a file is shared between models. Its path is locked while it is fetched, so the first model downloads it and the others reuse it.
pub struct Shared {
    /// When this path exists once the lock is held, whoever held it before did all the work: skip the file and its step.
    pub skip_if: Option<PathBuf>,
    pub then: Option<Step>,
}

/// One file of a download: where it comes from, its pinned size and hash, the folder it ends up in, and the extra room unpacking it takes.
pub struct Wanted {
    pub url: String,
    pub file: &'static ModelFile,
    pub dir: PathBuf,
    pub shared: Option<Shared>,
    pub unpacked: u64,
}

impl Wanted {
    /// A file only this model uses.
    pub fn own(url: String, file: &'static ModelFile, dir: PathBuf) -> Wanted {
        Wanted { url, file, dir, shared: None, unpacked: 0 }
    }

    /// Bytes this file still needs on disk: (to fetch, to unpack). A complete archive that was never unpacked still needs room to unpack; a file whose work was done by another model needs nothing.
    fn needed(&self) -> (u64, u64) {
        let len = |name: String| fs::metadata(self.dir.join(name)).map_or(0, |m| m.len());
        if self.shared.as_ref().and_then(|s| s.skip_if.as_ref()).is_some_and(|p| p.exists()) {
            return (0, 0);
        }
        let have = if len(self.file.name.into()) == self.file.size {
            self.file.size
        } else {
            len(format!("{}.partial", self.file.name)).min(self.file.size)
        };
        (self.file.size - have, self.unpacked)
    }
}

/// The sizes admission needs: everything, and what is left to write (with 5 % headroom).
fn sizes(wanted: &[Wanted]) -> Sizes {
    let total: u64 = wanted.iter().map(|w| w.file.size).sum();
    let (fetch, unpack) = wanted.iter().map(Wanted::needed).fold((0, 0), |(f, u), (a, b)| (f + a, u + b));
    let need = fetch + unpack;
    Sizes { total, need: need + need / 20 }
}

/// Works out what a download of one model needs now. Called again once the download is admitted, because a delete or another download may have changed the folder meanwhile.
pub type Plan = Box<dyn Fn() -> Result<Vec<Wanted>, String> + Send>;

/// The handle a page needs to cancel or recognise one download.
#[derive(Serialize)]
pub struct Token {
    pub id: &'static str,
    pub generation: u64,
}

#[derive(Serialize, Clone)]
struct Progress {
    id: &'static str,
    generation: u64,
    done: u64,
    total: u64,
}

#[derive(Serialize, Clone)]
struct Finished {
    id: &'static str,
    generation: u64,
    revision: u64,
    outcome: &'static str,
    error: Option<String>,
}

/// Bytes free on the drive that holds `dir` (or its nearest existing parent); unlimited when Windows cannot say, so a failed query never blocks a download.
fn free_room(dir: &Path) -> u64 {
    dir.ancestors().find(|d| d.exists()).and_then(|d| crate::platform::free_bytes(d).ok()).unwrap_or(u64::MAX)
}

/// Starts downloading what `plan` lists for model `id` of `group` ("speech", "text" or "dictionary") on its own thread; `after` runs when every file is in place.
/// Refused when `id` already downloads, is being deleted, or the drive lacks room.
pub fn start(app: &AppHandle, id: &'static str, group: &'static str, plan: Plan, after: After) -> Result<Token, String> {
    let first = plan()?;
    let planned = sizes(&first);
    let planned_need = planned.need;
    let admitted = MANAGER.admit(id, group, planned, &|| first.first().map_or(u64::MAX, |w| free_room(&w.dir)))?;
    let (generation, ctl) = (admitted.generation, admitted.ctl);
    let _ = app.emit("model-changed", serde_json::json!({ "revision": admitted.revision }));
    let handle = app.clone();
    let spawned = crate::guard::spawn("download", 256 * 1024, move || {
        let outcome = crate::guard::contain(|| execute(&handle, id, generation, &ctl, (&plan, planned_need), after))
            .unwrap_or_else(|| Outcome::Failed("The download stopped unexpectedly.".into()));
        conclude(&handle, id, generation, outcome);
    });
    if let Err(e) = spawned {
        MANAGER.finish(id, generation);
        return Err(e.to_string());
    }
    Ok(Token { id, generation })
}

/// Plans again now that the download is admitted (a delete or another download may have changed the folder meanwhile), asks for any room the new plan needs beyond the admitted one, and runs it.
fn execute(app: &AppHandle, id: &'static str, generation: u64, ctl: &state::Ctl, planned: (&Plan, u64), after: After) -> Outcome {
    let wanted = match planned.0() {
        Ok(wanted) => wanted,
        Err(e) => return Outcome::Failed(e),
    };
    let now = sizes(&wanted).need;
    if now > planned.1 {
        if let Err(e) = MANAGER.grow(id, generation, now - planned.1, &|| wanted.first().map_or(u64::MAX, |w| free_room(&w.dir))) {
            return Outcome::Failed(e);
        }
    }
    worker::run(&MANAGER, app, &wanted, after, ctl, &reporter(app, id, generation, ctl))
}

/// The progress callback of one download: remembers the bytes for snapshots and sends an event when the percent changes.
fn reporter<'a>(app: &'a AppHandle, id: &'static str, generation: u64, ctl: &'a state::Ctl) -> impl Fn(u64, u64) + 'a {
    let last_percent = Cell::new(u64::MAX);
    move |done, total| {
        ctl.done.store(done, Ordering::Relaxed);
        let percent = done * 100 / total.max(1);
        if last_percent.replace(percent) != percent {
            let _ = app.emit("model-progress", Progress { id, generation, done, total });
        }
    }
}

/// Ends a download: the table is updated first, then the user is told, so a page that asks for the snapshot on hearing "finished" already sees it gone.
fn conclude(app: &AppHandle, id: &'static str, generation: u64, outcome: Outcome) {
    let revision = MANAGER.finish(id, generation);
    let (name, error) = match outcome {
        Outcome::Succeeded => ("succeeded", None),
        Outcome::Cancelled => ("cancelled", None),
        Outcome::Failed(e) => ("failed", Some(e)),
    };
    if let Some(e) = &error {
        status::report_in_app(app, e);
    }
    let _ = app.emit("model-finished", Finished { id, generation, revision, outcome: name, error });
}

/// Tells every window the download table changed (a delete finished), so each reads it again.
pub fn announce(app: &AppHandle) {
    let _ = app.emit("model-changed", serde_json::json!({ "revision": MANAGER.view().revision }));
}

/// Stops the download of `id` when it is still generation `generation`.
pub fn cancel(id: &str, generation: u64) {
    MANAGER.cancel(id, generation);
}

/// Reserves `id` for deleting; see `Manager::reserve_delete`.
pub fn reserve_delete(id: &'static str, group: &'static str) -> Result<DeleteGuard, String> {
    MANAGER.reserve_delete(id, group)
}

/// The running downloads and the revision, for the Models pages.
pub fn view() -> View {
    MANAGER.view()
}

/// Stops every download and waits briefly for the threads; partial files stay for the next start.
pub fn shutdown() {
    MANAGER.shutdown(Duration::from_secs(3));
}

/// Tidies installs a quit or a crash left half done. Starts no download.
pub fn recover(app: &AppHandle) {
    crate::text::catalog::recover(app);
    crate::dictionary::catalog::recover(app);
}
