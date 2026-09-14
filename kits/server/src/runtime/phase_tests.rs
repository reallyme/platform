// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
