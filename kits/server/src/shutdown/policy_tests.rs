// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;

use super::{ShutdownMode, ShutdownPolicy};
use crate::shutdown::ShutdownReason;

#[test]
fn sigterm_maps_to_graceful_by_default() {
    assert_eq!(
        ShutdownPolicy::default().mode_for(ShutdownReason::Sigterm),
        ShutdownMode::Graceful
    );
}

#[test]
fn ctrl_c_maps_to_graceful_by_default() {
    assert_eq!(
        ShutdownPolicy::default().mode_for(ShutdownReason::CtrlC),
        ShutdownMode::Graceful
    );
}

#[test]
fn ctrl_c_can_be_configured_as_fast() {
    let policy = ShutdownPolicy::default().with_ctrl_c_mode(ShutdownMode::Fast);

    assert_eq!(policy.mode_for(ShutdownReason::CtrlC), ShutdownMode::Fast);
    assert_eq!(
        policy.mode_for(ShutdownReason::Sigterm),
        ShutdownMode::Graceful
    );
}

#[test]
fn shutdown_mode_parse_is_typed() {
    assert_eq!(
        ShutdownMode::from_str("graceful"),
        Ok(ShutdownMode::Graceful)
    );
    assert_eq!(ShutdownMode::from_str("fast"), Ok(ShutdownMode::Fast));
    assert!(ShutdownMode::from_str("immediate").is_err());
}
