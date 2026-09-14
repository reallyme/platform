// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tokio::sync::watch;

/// Typed readiness state used to decide whether a service should receive new
/// traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    /// The service is ready to receive traffic.
    Ready,
    /// The service should not receive traffic.
    NotReady,
}

/// Shared readiness controller.
///
/// Readiness transitions are intentionally explicit because they gate whether a
/// process should receive new traffic. The underlying atomic is private so the
/// crate can evolve the representation without leaking concurrency details.
///
/// This type intentionally uses `Ordering::SeqCst` for readability and
/// conservatism. Readiness changes are not expected to be hot enough to justify
/// more subtle memory ordering semantics at this stage of the platform.
///
/// Future dependency readiness checks can be layered on top of this controller
/// without changing the stable transport response model exposed by this module.
#[derive(Debug, Clone)]
pub struct Readiness {
    is_ready: Arc<AtomicBool>,
    state_sender: watch::Sender<ReadinessState>,
}

impl Default for Readiness {
    fn default() -> Self {
        Self::new()
    }
}

impl Readiness {
    /// Creates a readiness controller in the `not ready` state.
    pub fn new() -> Self {
        let (state_sender, _state_receiver) = watch::channel(ReadinessState::NotReady);
        Self {
            is_ready: Arc::new(AtomicBool::new(false)),
            state_sender,
        }
    }

    /// Sets the readiness state explicitly.
    pub fn set_state(&self, state: ReadinessState) {
        self.is_ready
            .store(matches!(state, ReadinessState::Ready), Ordering::SeqCst);
        let _ = self.state_sender.send(state);
    }

    /// Sets the readiness state explicitly by boolean value.
    pub fn set_ready(&self, ready: bool) {
        self.set_state(if ready {
            ReadinessState::Ready
        } else {
            ReadinessState::NotReady
        });
    }

    /// Marks the process as able to receive new traffic.
    pub fn mark_ready(&self) {
        self.set_state(ReadinessState::Ready);
    }

    /// Marks the process as unable to receive new traffic.
    pub fn mark_not_ready(&self) {
        self.set_state(ReadinessState::NotReady);
    }

    /// Returns the current typed readiness state.
    pub fn state(&self) -> ReadinessState {
        if self.is_ready.load(Ordering::SeqCst) {
            ReadinessState::Ready
        } else {
            ReadinessState::NotReady
        }
    }

    /// Returns whether the service is ready to receive traffic.
    pub fn is_ready(&self) -> bool {
        matches!(self.state(), ReadinessState::Ready)
    }

    /// Subscribes to readiness transitions.
    pub fn watch(&self) -> ReadinessWatcher {
        ReadinessWatcher {
            receiver: self.state_sender.subscribe(),
        }
    }
}

/// Read-only readiness transition watcher.
pub struct ReadinessWatcher {
    receiver: watch::Receiver<ReadinessState>,
}

impl ReadinessWatcher {
    /// Returns the latest observed readiness state.
    pub fn current(&self) -> ReadinessState {
        *self.receiver.borrow()
    }

    /// Waits for the next readiness state change.
    pub async fn changed(&mut self) -> Result<ReadinessState, ReadinessWatchError> {
        self.receiver
            .changed()
            .await
            .map_err(|_| ReadinessWatchError::Closed)?;
        Ok(self.current())
    }
}

/// Readiness watcher failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadinessWatchError {
    /// The readiness publisher was dropped.
    Closed,
}

#[cfg(test)]
#[path = "readiness_tests.rs"]
mod tests;
