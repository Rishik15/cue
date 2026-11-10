//! File: model_download/mod.rs
//! Purpose: Download models (speech, text, dictionary packs) side by side: every start runs at once on its own short-lived thread; nothing queues and nothing is capped.
//! Contents: Wanted / Shared / After — what a model needs; start / cancel / reserve_delete / view / shutdown — the interface the commands use (`state`, `ctl` and `ledger` keep the books, `job` and `worker` run a download,
//! `transfer` fetches one file, `http` talks HTTPS, `interrupt` ends blocked network calls on cancel); recover — tidy half-finished installs at launch.
//! Seams: `net` (HTTPS), `disk` (room) and `sink` (events) each have a production adapter and a test fake; `job` runs a download against them.
//! Events: `model-progress {id, generation, done, total}`, `model-finished {id, generation, revision, outcome, error}`, `model-changed {revision}` (a download started or a delete took place).
//! Cancel: one `Ctl` per download holds the flag and the interrupts each phase registers (socket close, unpacker kill), so Cancel and Quit stop every phase at once.
//! shortcut: all models live on one drive (the app data folder), so disk room is one number; split it per drive if models ever get a user-chosen folder.

mod child;
mod ctl;
mod disk;
mod http;
mod interrupt;
mod job;
mod ledger;
mod net;
mod resume;
mod sink;
mod state;
mod transfer;
mod worker;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::AppHandle;

pub use child::run_child;
pub use ctl::Ctl;
pub use state::{DeleteGuard, Last, View};

use crate::speech::catalog::ModelFile;
use disk::RealDisk;
use job::Env;
use ledger::Claim;
use sink::{Sink, TauriSink};
use state::{Manager, Sizes};

static MANAGER: Manager = Manager::new();

/// What runs once every file is in place (for example unpacking an archive).
pub type After = Box<dyn FnOnce(&Ctl) -> Result<(), String> + Send>;

/// A step run while the lock of a shared file is held, right after that file arrived.
pub type Step = Arc<dyn Fn(&Ctl) -> Result<(), String> + Send + Sync>;

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

    /// What this file claims from the disk books now: the bytes to fetch and the bytes to unpack, each under its own path (so a shared archive counts once), with 5 % headroom.
    pub(super) fn claims(&self) -> Vec<Claim> {
        let (fetch, unpack) = self.needed();
        let key = ledger::path_key(&self.dir, self.file.name);
        let padded = |bytes: u64| bytes + bytes / 20;
        vec![Claim { key: format!("{key}#unpack"), bytes: padded(unpack) }, Claim { key, bytes: padded(fetch) }]
    }
}

/// The sizes admission needs: the total, and what is left to write per path.
fn sizes(wanted: &[Wanted]) -> Sizes {
    Sizes { total: wanted.iter().map(|w| w.file.size).sum(), claims: wanted.iter().flat_map(Wanted::claims).collect() }
}

/// Works out what a download of one model needs now. Called again once the download is admitted, because a delete or another download may have changed the folder meanwhile.
pub type Plan = Box<dyn Fn() -> Result<Vec<Wanted>, String> + Send>;

/// The handle a page needs to cancel or recognise one download.
#[derive(Serialize)]
pub struct Token {
    pub id: &'static str,
    pub generation: u64,
}

/// The real network, drive and windows, with the process-wide table.
fn production(app: &AppHandle) -> Env {
    Env { manager: &MANAGER, net: Arc::new(http::HttpNet), disk: Arc::new(RealDisk), sink: Arc::new(TauriSink(app.clone())) }
}

/// Starts downloading what `plan` lists for model `id` of `group` ("speech", "text" or "dictionary") on its own thread; `after` runs when every file is in place.
/// Refused when `id` already downloads, is being deleted, or the drive lacks room.
pub fn start(app: &AppHandle, id: &'static str, group: &'static str, plan: Plan, after: After) -> Result<Token, String> {
    let env = production(app);
    let first = plan()?;
    let admitted = MANAGER.admit(id, group, sizes(&first), &|| first.first().map_or(u64::MAX, |w| env.disk.free_room(&w.dir)))?;
    let (generation, ctl) = (admitted.generation, admitted.ctl);
    env.sink.changed(admitted.revision);
    let spawned = crate::guard::spawn("download", 256 * 1024, move || job::run(&env, id, generation, &ctl, &plan, after));
    if let Err(e) = spawned {
        MANAGER.finish(id, generation);
        return Err(e.to_string());
    }
    Ok(Token { id, generation })
}

/// Tells every window the download table changed (a delete finished), so each reads it again.
pub fn announce(app: &AppHandle) {
    TauriSink(app.clone()).changed(MANAGER.view().revision);
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

#[cfg(test)]
mod tests;
