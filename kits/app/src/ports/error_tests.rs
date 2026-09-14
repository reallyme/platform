// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppPortError;

#[test]
fn port_error_metric_labels_are_stable() {
    assert_eq!(AppPortError::Unconfigured.as_metric_label(), "unconfigured");
    assert_eq!(AppPortError::Timeout.as_metric_label(), "timeout");
}
