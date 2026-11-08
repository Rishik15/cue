//! File: runtime.rs
//! Purpose: Gives Tauri the async runtime Cue wants: two worker threads instead of one per processor, and blocking threads that leave after two idle seconds. Tauri starts its runtime for the first window page it serves, so how many threads that start decides how many Cue keeps after Settings closes.
//! Contents: ensure — build the runtime and hand it to Tauri, once, just before the first window is built.
//! The runtime is built lazily because a Tokio runtime starts its workers when built: building it at launch would add two idle threads to a tray-only Cue. Every code path that builds a window calls `ensure` first.
//! Work that waits on the disk, a dialog or the main thread runs on the blocking pool (`spawn_blocking`), never on the two workers, so the page that is loading is not held up behind it.

use std::sync::Once;
use std::time::Duration;

/// Worker threads that poll futures; blocking work does not use them.
const WORKERS: usize = 2;
/// Stack of every runtime thread. The blocking pool runs OCR language lookups, SQLite and dialogs, which need more than a worker.
const STACK: usize = 1024 * 1024;
/// How long an idle blocking thread stays before it exits.
const BLOCKING_KEEP_ALIVE: Duration = Duration::from_secs(2);
/// The most blocking threads at once; downloads, imports and saves queue behind it instead of starting hundreds of threads.
const BLOCKING_MAX: usize = 16;

static ONCE: Once = Once::new();

/// Builds the runtime and gives it to Tauri. Returns Err with the reason when the runtime cannot be built; panics (from the thread pool, or from Tauri when it already has a runtime) are for the caller to contain.
fn build_and_install() -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKERS)
        .max_blocking_threads(BLOCKING_MAX)
        .thread_keep_alive(BLOCKING_KEEP_ALIVE)
        .thread_stack_size(STACK)
        .thread_name("cue-async")
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::set(runtime.handle().clone());
    std::mem::forget(runtime);
    Ok(())
}

/// Builds the runtime and gives it to Tauri the first time it is called; later calls do nothing. Call it before building any window. The runtime is kept for the life of the process, as Tauri requires. If it cannot be built, or Tauri already started its own (a path that forgot to call this), or building panics, Tauri's default runtime is used and Cue only loses the thread savings; a failure never reaches the window being built.
pub fn ensure() {
    ONCE.call_once(|| match std::panic::catch_unwind(build_and_install) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => crate::log::note(&format!("runtime: could not build Cue's runtime, Tauri will use its own: {e}")),
        Err(_) => crate::log::note("runtime: Cue's runtime could not start, Tauri will use its own"),
    });
}
