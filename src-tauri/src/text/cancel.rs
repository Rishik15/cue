//! File: text/cancel.rs
//! Purpose: The signal that a caller no longer wants a text model call, so a cold start or a request stops at once and gives the model back.
//! Contents: Cancel — a cloneable, thread-safe "is it still wanted" check; `new` wraps any check, `never` is the check of a caller that cannot be cancelled.

use std::sync::Arc;

/// A cancel token: asking it is cheap and can be done from any thread. The text engine asks between the steps of a start and while it waits for an answer.
#[derive(Clone)]
pub struct Cancel(Arc<dyn Fn() -> bool + Send + Sync>);

impl Cancel {
    /// Wraps `check`, which returns true once the call is no longer wanted. Returns the token.
    pub fn new(check: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(check))
    }

    /// A token that is never set, for callers that cannot be cancelled. Returns the token.
    pub fn never() -> Self {
        Self::new(|| false)
    }

    /// Whether the call is no longer wanted. Returns true when it has been cancelled.
    pub fn is_set(&self) -> bool {
        (self.0)()
    }
}
