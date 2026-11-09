//! File: model_download/job.rs
//! Purpose: The life of one admitted download from the thread's first line to its end: plan again, book the disk, run the steps, tell the table and the windows how it ended. Takes its network, drive and announcements through `Env`, so tests run it against fakes.
//! Contents: Env — the seams and the table; run — the thread body of one download.

use std::cell::Cell;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::disk::Disk;
use super::net::Net;
use super::sink::{Finished, Progress, Sink};
use super::state::{Ctl, Manager, Phase};
use super::worker::{self, Outcome};
use super::{sizes, After, Plan};

/// Everything a download talks to besides its own files.
pub struct Env {
    pub manager: &'static Manager,
    pub net: Arc<dyn Net>,
    pub disk: Arc<dyn Disk>,
    pub sink: Arc<dyn Sink>,
}

/// Runs the download `id` of `generation` to its end and announces the end. A panic inside is contained and counts as a failure.
pub fn run(env: &Env, id: &'static str, generation: u64, ctl: &Ctl, plan: &Plan, after: After) {
    let (manager, sink) = (env.manager, env.sink.clone());
    ctl.watch_phase(Box::new(move || sink.changed(manager.touch())));
    let outcome = crate::guard::contain(|| execute(env, id, generation, ctl, plan, after))
        .unwrap_or_else(|| Outcome::Failed("The download stopped unexpectedly.".into()));
    conclude(env, id, generation, outcome);
}

/// Plans again now that the download is admitted (a delete or another download may have changed the folder meanwhile), asks the books for exactly what the new plan needs, and runs it.
fn execute(env: &Env, id: &'static str, generation: u64, ctl: &Ctl, plan: &Plan, after: After) -> Outcome {
    let wanted = match plan() {
        Ok(wanted) => wanted,
        Err(e) => return Outcome::Failed(e),
    };
    let free = || wanted.first().map_or(u64::MAX, |w| env.disk.free_room(&w.dir));
    if let Err(e) = ctl.retarget(&sizes(&wanted).claims, &free) {
        return Outcome::Failed(e);
    }
    ctl.set_phase(Phase::Fetching);
    worker::run(env, &wanted, after, ctl, &reporter(env, id, generation, ctl))
}

/// The progress callback of one download: remembers the bytes for snapshots and sends an event when the percent changes.
fn reporter<'a>(env: &'a Env, id: &'static str, generation: u64, ctl: &'a Ctl) -> impl Fn(u64, u64) + 'a {
    let last_percent = Cell::new(u64::MAX);
    move |done, total| {
        ctl.done.store(done, Ordering::Relaxed);
        let percent = done * 100 / total.max(1);
        if last_percent.replace(percent) != percent {
            env.sink.progress(Progress { id, generation, done, total });
        }
    }
}

/// Ends a download: the table is updated first, then the user is told, so a page that asks for the snapshot on hearing "finished" already sees it gone.
fn conclude(env: &Env, id: &'static str, generation: u64, outcome: Outcome) {
    let (name, error) = match outcome {
        Outcome::Succeeded => ("succeeded", None),
        Outcome::Cancelled => ("cancelled", None),
        Outcome::Failed(e) => ("failed", Some(e)),
    };
    let (revision, outcome) = env.manager.conclude(id, generation, name, error.clone());
    env.sink.finished(Finished { id, generation, revision, outcome, error });
}
