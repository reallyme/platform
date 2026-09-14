// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{AppPortTimeout, AppPortTimeoutError};

#[test]
fn timeout_rejects_zero_duration() {
    assert_eq!(
        AppPortTimeout::new(Duration::ZERO),
        Err(AppPortTimeoutError::Zero)
    );
}

#[test]
fn timeout_accepts_non_zero_duration() {
    let timeout = AppPortTimeout::new(Duration::from_millis(1))
        .expect("non-zero timeout fixture should be valid");

    assert_eq!(timeout.duration(), Duration::from_millis(1));
}
