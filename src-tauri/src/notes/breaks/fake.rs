//! File: notes/breaks/fake.rs
//! Purpose: A stand-in for the break window and streak settings, so the break lifecycle runs in tests without an app.
//! Contents: Fake — a `Stage` that records whether its window is open and the streak it was told to save.

use std::cell::{Cell, RefCell};

use super::{Owner, Stage};

/// A break stage in memory.
#[derive(Default)]
pub struct Fake {
    open: Cell<bool>,
    streak: Cell<u64>,
    reports: RefCell<Vec<String>>,
}

impl Fake {
    /// Is the fake window open.
    pub fn open(&self) -> bool {
        self.open.get()
    }
}

impl Stage for Fake {
    /// Is the fake window open.
    fn exists(&self) -> bool {
        self.open.get()
    }

    /// Closes the fake window.
    fn close(&self) {
        self.open.set(false);
    }

    /// Opens the fake window.
    fn build(&self, _: Owner, _: u64, _: u32) -> Result<(), String> {
        self.open.set(true);
        Ok(())
    }

    /// The streak last saved.
    fn streak(&self) -> u64 {
        self.streak.get()
    }

    /// Remembers the streak.
    fn save(&self, streak: u64) {
        self.streak.set(streak);
    }

    /// Remembers the message.
    fn report(&self, message: &str) {
        self.reports.borrow_mut().push(message.into());
    }
}
