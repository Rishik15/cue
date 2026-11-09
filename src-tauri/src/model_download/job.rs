//! File: model_download/job.rs
//! Purpose: The life of one admitted download from the thread's first line to its end: plan again, book the disk, run the steps, tell the table and the windows how it ended. Takes its network, drive and announcements through `Env`, so tests run it against fakes.
//! Contents: Env — the seams and the table; run — the thread body of one download.

use std::cell::Cell;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::ctl::{Ctl, Phase};
use super::disk::Disk;
use super::net::Net;
use super::sink::{Finished, Progress, Sink};
use super::state::{Manager, Outcome};
use super::transfer::Stop;
use super::worker;
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
    let result = crate::guard::contain(|| execute(env, id, generation, ctl, plan, after));
    conclude(env, id, generation, result);
}

/// Plans again now that the download is admitted (a delete or another download may have changed the folder meanwhile), books the disk for exactly what the new plan needs, and runs it.
fn execute(env: &Env, id: &'static str, generation: u64, ctl: &Ctl, plan: &Plan, after: After) -> Result<(), Stop> {
    let wanted = plan().map_err(Stop::Failed)?;
    let planned = sizes(&wanted);
    let free = || wanted.first().map_or(u64::MAX, |w| env.disk.free_room(&w.dir));
    ctl.retarget(&planned.claims, &free).map_err(Stop::Failed)?;
    ctl.set_total(planned.total);
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

/// Ends a download: the table is updated first, then the user is told, so a page that asks for the snapshot on hearing "finished" already sees it gone. `None` is a panic.
fn conclude(env: &Env, id: &'static str, generation: u64, result: Option<Result<(), Stop>>) {
    let (outcome, error) = match result {
        Some(Ok(())) => (Outcome::Succeeded, None),
        Some(Err(Stop::Cancelled)) => (Outcome::Cancelled, None),
        Some(Err(Stop::Failed(e) | Stop::Offline(e))) => (Outcome::Failed, Some(e)),
        None => (Outcome::Failed, Some("The download stopped unexpectedly.".to_string())),
    };
    let (revision, outcome) = env.manager.conclude(id, generation, outcome, error.clone());
    env.sink.finished(Finished { id, generation, revision, outcome, error });
}
