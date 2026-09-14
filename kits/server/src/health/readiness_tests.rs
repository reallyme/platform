// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Readiness, ReadinessState};

#[test]
fn readiness_starts_not_ready() {
    let readiness = Readiness::new();

    assert_eq!(readiness.state(), ReadinessState::NotReady);
    assert!(!readiness.is_ready());
}

#[test]
fn readiness_can_transition_to_ready() {
    let readiness = Readiness::new();

    readiness.mark_ready();

    assert_eq!(readiness.state(), ReadinessState::Ready);
    assert!(readiness.is_ready());
}

#[test]
fn readiness_can_transition_back_to_not_ready() {
    let readiness = Readiness::new();
    readiness.mark_ready();

    readiness.mark_not_ready();

    assert_eq!(readiness.state(), ReadinessState::NotReady);
    assert!(!readiness.is_ready());
}

#[tokio::test]
async fn readiness_watcher_observes_transitions() {
    let readiness = Readiness::new();
    let mut watcher = readiness.watch();

    assert_eq!(watcher.current(), ReadinessState::NotReady);

    readiness.mark_ready();

    assert_eq!(
        watcher
            .changed()
            .await
            .expect("readiness publisher should remain open"),
        ReadinessState::Ready
    );
}
