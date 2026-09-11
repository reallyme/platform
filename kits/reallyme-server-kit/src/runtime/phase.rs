// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;
use thiserror::Error;
use tokio::sync::{broadcast, watch};

/// Observable lifecycle phase for a [`crate::runtime::ServerRuntime`].
///
/// These phases are deliberately low-cardinality and service-agnostic. They
/// exist so tests, operators, and future supervision code can reason about the
/// runtime lifecycle without scraping logs or relying on timing sleeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerRuntimePhase {
    /// Startup validation and observability initialization are in progress.
    Initializing,
    /// Transport listeners are being bound.
    BindingListeners,
    /// Managed transport and background tasks are being registered.
    StartingBackgroundTasks,
    /// The service has been marked ready to receive traffic.
    Serving,
    /// The service has been marked not-ready and managed tasks are draining.
    Draining,
    /// Final shutdown work is running after the drain budget has completed.
    ShuttingDown,
    /// The runtime stopped successfully.
    Stopped,
    /// Runtime startup or shutdown failed.
    Failed,
}

impl ServerRuntimePhase {
    /// Returns the stable structured-log and metric label value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Initializing => "initializing",
            Self::BindingListeners => "binding_listeners",
            Self::StartingBackgroundTasks => "starting_background_tasks",
            Self::Serving => "serving",
            Self::Draining => "draining",
            Self::ShuttingDown => "shutting_down",
            Self::Stopped => "stopped",
            Self::Failed => "failed",
        }
    }

    /// Returns a stable numeric value for gauge-style metrics.
    pub const fn metric_value(self) -> f64 {
        match self {
            Self::Initializing => 0.0,
            Self::BindingListeners => 1.0,
            Self::StartingBackgroundTasks => 2.0,
            Self::Serving => 3.0,
            Self::Draining => 4.0,
            Self::ShuttingDown => 5.0,
            Self::Stopped => 6.0,
            Self::Failed => 7.0,
        }
    }
}

/// Runtime phase watcher failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ServerRuntimePhaseWatchError {
    /// The runtime phase publisher was dropped.
    #[error("server runtime phase publisher closed")]
    Closed,
    /// The transition subscriber lagged and missed one or more events.
    #[error("server runtime phase subscriber lagged")]
    Lagged {
        /// Number of skipped phase transition events.
        skipped: u64,
    },
}

/// Read-only runtime lifecycle phase subscription.
///
/// The watcher exposes the current phase through a watch channel and ordered
/// transition events through a bounded broadcast channel. App crates can
/// observe phases for tests or background coordination, but cannot mutate
/// runtime phase state.
pub struct ServerRuntimePhaseWatcher {
    current: watch::Receiver<ServerRuntimePhase>,
    transitions: broadcast::Receiver<ServerRuntimePhase>,
}

impl ServerRuntimePhaseWatcher {
    /// Returns the latest observed runtime phase.
    pub fn current(&self) -> ServerRuntimePhase {
        *self.current.borrow()
    }

    /// Waits until the current runtime phase changes and returns the new value.
    pub async fn changed(&mut self) -> Result<ServerRuntimePhase, ServerRuntimePhaseWatchError> {
        self.current
            .changed()
            .await
            .map_err(|_| ServerRuntimePhaseWatchError::Closed)?;
        Ok(self.current())
    }

    /// Waits for the next ordered phase transition event.
    pub async fn next_transition(
        &mut self,
    ) -> Result<ServerRuntimePhase, ServerRuntimePhaseWatchError> {
        self.transitions.recv().await.map_err(|error| match error {
            broadcast::error::RecvError::Closed => ServerRuntimePhaseWatchError::Closed,
            broadcast::error::RecvError::Lagged(skipped) => {
                ServerRuntimePhaseWatchError::Lagged { skipped }
            }
        })
    }
}

/// Emits runtime lifecycle phase transitions.
///
/// The reporter is cloneable so it can be passed through runtime construction,
/// but only `reallyme-server-kit` should publish transitions. App crates
/// should subscribe to the returned watch receiver for tests or future
/// supervision rather than mutating runtime state themselves.
#[derive(Debug, Clone)]
pub struct ServerRuntimePhaseReporter {
    current_sender: watch::Sender<ServerRuntimePhase>,
    transition_sender: broadcast::Sender<ServerRuntimePhase>,
}

impl Default for ServerRuntimePhaseReporter {
    fn default() -> Self {
        Self::new().0
    }
}

impl ServerRuntimePhaseReporter {
    /// Creates a new phase reporter and watch receiver.
    pub fn new() -> (Self, ServerRuntimePhaseWatcher) {
        let (current_sender, current_receiver) = watch::channel(ServerRuntimePhase::Initializing);
        let (transition_sender, transition_receiver) = broadcast::channel(32);
        (
            Self {
                current_sender,
                transition_sender,
            },
            ServerRuntimePhaseWatcher {
                current: current_receiver,
                transitions: transition_receiver,
            },
        )
    }

    /// Returns a new receiver subscribed to this reporter.
    pub fn subscribe(&self) -> ServerRuntimePhaseWatcher {
        ServerRuntimePhaseWatcher {
            current: self.current_sender.subscribe(),
            transitions: self.transition_sender.subscribe(),
        }
    }

    pub(crate) fn transition(&self, phase: ServerRuntimePhase) {
        let _ = self.current_sender.send(phase);
        let _ = self.transition_sender.send(phase);
    }
}

#[cfg(test)]
mod tests {
    use super::{ServerRuntimePhase, ServerRuntimePhaseReporter};

    #[tokio::test]
    async fn phase_reporter_emits_transitions() {
        let (reporter, mut watch) = ServerRuntimePhaseReporter::new();

        assert_eq!(watch.current(), ServerRuntimePhase::Initializing);

        reporter.transition(ServerRuntimePhase::BindingListeners);
        watch
            .changed()
            .await
            .expect("phase reporter should remain open");

        assert_eq!(watch.current(), ServerRuntimePhase::BindingListeners);
        assert_eq!(
            watch
                .next_transition()
                .await
                .expect("phase transition should be emitted"),
            ServerRuntimePhase::BindingListeners
        );
    }

    #[test]
    fn phase_values_are_low_cardinality_and_stable() {
        assert_eq!(ServerRuntimePhase::Initializing.as_str(), "initializing");
        assert_eq!(
            ServerRuntimePhase::BindingListeners.as_str(),
            "binding_listeners"
        );
        assert_eq!(
            ServerRuntimePhase::StartingBackgroundTasks.as_str(),
            "starting_background_tasks"
        );
        assert_eq!(ServerRuntimePhase::Serving.as_str(), "serving");
        assert_eq!(ServerRuntimePhase::Draining.as_str(), "draining");
        assert_eq!(ServerRuntimePhase::ShuttingDown.as_str(), "shutting_down");
        assert_eq!(ServerRuntimePhase::Stopped.as_str(), "stopped");
        assert_eq!(ServerRuntimePhase::Failed.as_str(), "failed");
    }
}
