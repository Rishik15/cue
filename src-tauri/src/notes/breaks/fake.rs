//! File: notes/breaks/fake.rs
//! Purpose: A stand-in for the break window and streak settings, so the break lifecycle runs in tests without an app.
//! Contents: Fake — a `Stage` that records whether its window is open, the streak it was told to save and the messages it was told to show.

use std::cell::{Cell, RefCell};

use super::Stage;

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

    /// The messages shown to the user so far.
    pub fn reports(&self) -> Vec<String> {
        self.reports.borrow().clone()
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
    fn build(&self, _: u64, _: u64, _: u32) -> Result<(), String> {
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
