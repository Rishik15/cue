//! File: model_download/net.rs
//! Purpose: The seam between a download and the network: one call that opens a file from a byte on. The production adapter is `http::HttpNet`; tests plug in a scripted one.
//! Contents: Net — the trait; Fetched — what an open returned.

use std::io::Read;

use super::state::Ctl;
use super::transfer::Stop;

/// What the server answered for a request that may be read: its status, where a partial answer starts, and the body.
pub struct Fetched {
    pub status: u16,
    pub content_range: Option<String>,
    pub body: Box<dyn Read + Send>,
}

/// Opens files over the network. An implementation registers what it must close on `ctl` so a cancel ends a blocked call.
pub trait Net: Send + Sync {
    /// Asks for `url` from byte `have` on. `Ok(None)`: the server says the range does not fit the file (HTTP 416), so the caller starts over.
    fn open(&self, url: &str, have: u64, ctl: &Ctl) -> Result<Option<Fetched>, Stop>;
}
