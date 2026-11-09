//! File: model_download/tests/fakes.rs
//! Purpose: The test adapters of the download seams (`Net`, `Disk`, `Sink`) and a rig that runs one download against them on a table of its own.
//! Contents: Recorder — a Sink that keeps events; FakeDisk — scripted room; FakeNet — scripted answers, with helpers for bodies that hang until a cancel; Rig — admit and run a job.

use std::io::{self, Cursor, Read};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::model_download::disk::Disk;
use crate::model_download::job::{self, Env};
use crate::model_download::net::{Fetched, Net};
use crate::model_download::sink::{Finished, Progress, Sink};
use crate::model_download::state::{Ctl, Manager};
use crate::model_download::transfer::Stop;
use crate::model_download::{sizes, After, Plan, Wanted};

/// One announcement a download made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Progress(Progress),
    Finished(Finished),
    Changed(u64),
}

/// A Sink that keeps everything it is told, in order.
#[derive(Default)]
pub struct Recorder {
    events: Mutex<Vec<Event>>,
}

impl Recorder {
    /// Everything announced so far.
    pub fn events(&self) -> Vec<Event> {
        self.events.lock().expect("events").clone()
    }

    /// The end of the download, when it was announced.
    pub fn finished(&self) -> Option<Finished> {
        self.events().into_iter().find_map(|e| if let Event::Finished(f) = e { Some(f) } else { None })
    }
}

impl Sink for Recorder {
    /// Keeps the event.
    fn progress(&self, event: Progress) {
        self.events.lock().expect("events").push(Event::Progress(event));
    }

    /// Keeps the event.
    fn finished(&self, event: Finished) {
        self.events.lock().expect("events").push(Event::Finished(event));
    }

    /// Keeps the event.
    fn changed(&self, revision: u64) {
        self.events.lock().expect("events").push(Event::Changed(revision));
    }
}

/// A drive with a fixed amount of room whose folders are made on the real temp folder.
pub struct FakeDisk {
    pub free: u64,
}

impl Disk for FakeDisk {
    /// The scripted room.
    fn free_room(&self, _dir: &Path) -> u64 {
        self.free
    }

    /// Creates the folder for real: tests write into a temp folder.
    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)
    }
}

/// How a scripted network answers one open: the url, the byte asked from, and the cancel object.
pub type Script = dyn Fn(&str, u64, &Ctl) -> Result<Option<Fetched>, Stop> + Send + Sync;

/// A network whose answers a test writes; it remembers every request.
pub struct FakeNet {
    script: Box<Script>,
    calls: Mutex<Vec<(String, u64)>>,
}

impl FakeNet {
    /// A network that answers with `script`.
    pub fn new(script: impl Fn(&str, u64, &Ctl) -> Result<Option<Fetched>, Stop> + Send + Sync + 'static) -> FakeNet {
        FakeNet { script: Box::new(script), calls: Mutex::new(Vec::new()) }
    }

    /// The requests made so far: url and start byte.
    pub fn calls(&self) -> Vec<(String, u64)> {
        self.calls.lock().expect("calls").clone()
    }
}

impl Net for FakeNet {
    /// Records the request and answers from the script.
    fn open(&self, url: &str, have: u64, ctl: &Ctl) -> Result<Option<Fetched>, Stop> {
        self.calls.lock().expect("calls").push((url.to_string(), have));
        (self.script)(url, have, ctl)
    }
}

/// A 200 answer carrying `bytes`.
pub fn whole(bytes: &[u8]) -> Fetched {
    Fetched { status: 200, content_range: None, body: Box::new(Cursor::new(bytes.to_vec())) }
}

/// A 206 answer carrying `bytes[from..]`.
pub fn resumed(bytes: &[u8], from: usize) -> Fetched {
    let range = format!("bytes {from}-{}/{}", bytes.len() - 1, bytes.len());
    Fetched { status: 206, content_range: Some(range), body: Box::new(Cursor::new(bytes[from..].to_vec())) }
}

/// Blocks until `ctl` is cancelled (woken by an interrupt, like a closed socket), after telling `entered` it is waiting.
pub fn hang(ctl: &Ctl, entered: &Sender<()>) {
    let (wake, woken) = channel::<()>();
    ctl.register(Box::new(move || {
        let _ = wake.send(());
    }));
    let _ = entered.send(());
    let _ = woken.recv_timeout(Duration::from_secs(30));
}

/// A body that delivers `first` and then blocks until the download is cancelled, as a silent connection does.
pub struct StallingBody {
    first: Cursor<Vec<u8>>,
    woken: Receiver<()>,
    entered: Sender<()>,
}

impl StallingBody {
    /// A stalling body after `first`; its wake-up is registered on `ctl` now, so a cancel at any moment ends the stall.
    pub fn new(first: Vec<u8>, ctl: &Ctl, entered: Sender<()>) -> StallingBody {
        let (wake, woken) = channel::<()>();
        ctl.register(Box::new(move || {
            let _ = wake.send(());
        }));
        StallingBody { first: Cursor::new(first), woken, entered }
    }
}

impl Read for StallingBody {
    /// Reads from `first`, then waits for the cancel and fails.
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.first.read(buf)?;
        if n > 0 {
            return Ok(n);
        }
        let _ = self.entered.send(());
        let _ = self.woken.recv_timeout(Duration::from_secs(30));
        Err(io::Error::other("connection closed"))
    }
}

/// A table of downloads of its own, a recorder and the seams, ready to run jobs.
pub struct Rig {
    pub env: Arc<Env>,
    pub net: Arc<FakeNet>,
    pub sink: Arc<Recorder>,
}

impl Rig {
    /// A rig whose network answers with `net` and whose drive has `free` bytes.
    pub fn new(net: FakeNet, free: u64) -> Rig {
        let (net, sink) = (Arc::new(net), Arc::new(Recorder::default()));
        let manager: &'static Manager = Box::leak(Box::new(Manager::new()));
        let env = Env { manager, net: net.clone(), disk: Arc::new(FakeDisk { free }), sink: sink.clone() };
        Rig { env: Arc::new(env), net, sink }
    }

    /// Admits model `id` with the plan and returns its generation and cancel object.
    pub fn admit(&self, id: &'static str, plan: &Plan) -> (u64, Arc<Ctl>) {
        let first = plan().expect("plan");
        let admitted = self.env.manager.admit(id, "speech", sizes(&first), &|| u64::MAX).expect("admitted");
        (admitted.generation, admitted.ctl)
    }

    /// Runs model `id` to its end on this thread.
    pub fn run(&self, id: &'static str, plan: Plan, after: After) {
        let (generation, ctl) = self.admit(id, &plan);
        job::run(&self.env, id, generation, &ctl, &plan, after);
    }

    /// Starts model `id` on its own thread; returns the generation, the cancel object, the thread, and a receiver that hears when the fake network or installer blocks.
    pub fn spawn(&self, id: &'static str, plan: Plan, after: After) -> (u64, Arc<Ctl>, std::thread::JoinHandle<()>) {
        let (generation, ctl) = self.admit(id, &plan);
        let (env, running) = (self.env.clone(), ctl.clone());
        let handle = std::thread::spawn(move || job::run(&env, id, generation, &running, &plan, after));
        (generation, ctl, handle)
    }
}

/// Waits until a blocking fake announced it is waiting.
pub fn await_entered(entered: &Receiver<()>) {
    entered.recv_timeout(Duration::from_secs(30)).expect("the fake never blocked");
}

/// A plan that lists one own file.
pub fn plan_of(url: &str, file: &'static crate::speech::catalog::ModelFile, dir: &Path) -> Plan {
    let (url, dir) = (url.to_string(), dir.to_path_buf());
    Box::new(move || Ok(vec![Wanted::own(url.clone(), file, dir.clone())]))
}
