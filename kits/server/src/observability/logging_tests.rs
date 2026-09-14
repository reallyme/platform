// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ErrorKind, shutdown_reason_value};
use crate::shutdown::ShutdownReason;

#[test]
fn error_kind_values_remain_stable() {
    assert_eq!(ErrorKind::Configuration.as_str(), "configuration");
    assert_eq!(ErrorKind::Transport.as_str(), "transport");
    assert_eq!(ErrorKind::Timeout.as_str(), "timeout");
    assert_eq!(ErrorKind::Authentication.as_str(), "authentication");
    assert_eq!(ErrorKind::Authorization.as_str(), "authorization");
    assert_eq!(
        ErrorKind::DependencyUnavailable.as_str(),
        "dependency_unavailable"
    );
    assert_eq!(ErrorKind::Internal.as_str(), "internal");
}

#[test]
fn shutdown_reason_values_remain_stable() {
    assert_eq!(shutdown_reason_value(ShutdownReason::CtrlC), "ctrl_c");
    assert_eq!(shutdown_reason_value(ShutdownReason::Sigterm), "sigterm");
    assert_eq!(shutdown_reason_value(ShutdownReason::Unknown), "unknown");
}
