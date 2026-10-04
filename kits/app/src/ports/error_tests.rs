// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppPortError;

#[test]
fn port_error_metric_labels_are_stable() {
    for (error, expected) in [
        (AppPortError::Unconfigured, "unconfigured"),
        (AppPortError::Timeout, "timeout"),
        (AppPortError::Unavailable, "unavailable"),
        (AppPortError::ProtocolViolation, "protocol_violation"),
        (AppPortError::NotImplemented, "not_implemented"),
    ] {
        assert_eq!(error.as_metric_label(), expected);
    }
}
