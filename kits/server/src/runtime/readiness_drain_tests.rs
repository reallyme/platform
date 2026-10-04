// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{ReadinessDrainDelay, ReadinessDrainDelayErrorReason};

#[test]
fn readiness_drain_delay_has_bounded_default_and_opt_out() {
    assert_eq!(
        ReadinessDrainDelay::default().as_duration(),
        Duration::from_secs(2)
    );
    assert_eq!(
        ReadinessDrainDelay::new(Duration::ZERO)
            .expect("explicit opt out is valid")
            .as_duration(),
        Duration::ZERO
    );
    assert!(matches!(
        ReadinessDrainDelay::new(Duration::from_secs(31)),
        Err(error) if error.reason() == ReadinessDrainDelayErrorReason::ExceedsMaximum
    ));
}
