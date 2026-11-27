//! File: update/phase.rs
//! Purpose: The update state and its pure transitions: what a check may change and what cleanup may delete are decided from state in, state out, with no app handle.
//! Contents: Phase / Snapshot / snapshot_of — the state and what windows see; Keep / Settled / settle — the outcome of a check; apply — change the state under its lock.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;

use super::manifest::{newer, Manifest};
use crate::lock::lock;

/// Where the update is: nothing found, downloading, ready on disk, or installing.
#[derive(Debug, PartialEq)]
pub(super) enum Phase {
    None,
    Downloading { manifest: Manifest, percent: u8 },
    Ready { manifest: Manifest, path: PathBuf },
    Installing,
}

/// What the windows see; also the payload of the `cue-update` event.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    phase: &'static str,
    version: Option<String>,
    percent: u8,
}

/// What cleanup of the update folder may delete.
#[derive(Debug, PartialEq)]
pub(super) enum Keep {
    All,
    File(String),
    Untouched,
}

/// The result of a check: the phase to switch to (if any) and what cleanup may delete.
#[derive(Debug, PartialEq)]
pub(super) struct Settled {
    pub next: Option<Phase>,
    pub keep: Keep,
}

/// The windows' view of a phase.
pub(super) fn snapshot_of(phase: &Phase) -> Snapshot {
    let (name, version, percent) = match phase {
        Phase::None => ("none", None, 0),
        Phase::Downloading { manifest, percent } => ("downloading", Some(manifest.version.clone()), *percent),
        Phase::Ready { manifest, .. } => ("ready", Some(manifest.version.clone()), 100),
        Phase::Installing => ("installing", None, 100),
    };
    Snapshot { phase: name, version, percent }
}

/// The phase for a release just found: Ready when its verified installer is on disk, else Downloading.
fn found_phase(found: &Manifest, on_disk: Option<PathBuf>) -> Phase {
    match on_disk {
        Some(path) => Phase::Ready { manifest: found.clone(), path },
        None => Phase::Downloading { manifest: found.clone(), percent: 0 },
    }
}

/// Decides what a finished check changes. A download or install in flight, and a Ready installer that nothing newer replaces, are left alone, file included.
pub(super) fn settle(now: &Phase, found: Option<&Manifest>, on_disk: Option<PathBuf>) -> Settled {
    match (now, found) {
        (Phase::Downloading { .. } | Phase::Installing, _) => Settled { next: None, keep: Keep::Untouched },
        (Phase::Ready { manifest, .. }, Some(f)) if newer(&f.version, &manifest.version) => {
            Settled { next: Some(found_phase(f, on_disk)), keep: Keep::File(f.version.clone()) }
        }
        (Phase::Ready { .. }, _) => Settled { next: None, keep: Keep::Untouched },
        (Phase::None, Some(f)) => Settled { next: Some(found_phase(f, on_disk)), keep: Keep::File(f.version.clone()) },
        (Phase::None, None) => Settled { next: None, keep: Keep::All },
    }
}

/// Replaces the phase when `change` returns one; returns the new snapshot, or None when nothing changed.
pub(super) fn apply(state: &Mutex<Phase>, change: impl FnOnce(&Phase) -> Option<Phase>) -> Option<Snapshot> {
    let mut state = lock(state);
    let next = change(&state)?;
    *state = next;
    Some(snapshot_of(&state))
}
