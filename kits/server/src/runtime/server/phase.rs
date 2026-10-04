// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared phase publication for runtime state, logs, and metrics.

use crate::observability::{log_runtime_phase_transition, record_runtime_phase};
use crate::startup::ServerName;

use super::super::phase::{ServerRuntimePhase, ServerRuntimePhaseReporter};

pub(super) fn transition_runtime_phase(
    phase_reporter: &ServerRuntimePhaseReporter,
    server_name: &ServerName,
    phase: ServerRuntimePhase,
) {
    phase_reporter.transition(phase);
    publish_runtime_phase(server_name, phase);
}

pub(super) fn publish_runtime_phase(server_name: &ServerName, phase: ServerRuntimePhase) {
    log_runtime_phase_transition(server_name, phase);
    record_runtime_phase(phase);
}
