// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppMetricName;
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn accepts_low_cardinality_metric_names() {
    let name = AppMetricName::new("precheck_requests").expect("valid metric name fixture");

    assert_eq!(name.as_str(), "precheck_requests");
}

#[test]
fn rejects_dynamic_looking_metric_names() {
    assert_eq!(
        AppMetricName::new("/v1/handles/alice"),
        Err(AppKitError::new(
            AppKitField::MetricName,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}
