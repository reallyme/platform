// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::observability::error::MetricLabelError;
use crate::observability::metrics::{HttpStatusClass, RouteTemplate};
#[cfg(all(feature = "http", feature = "tonic-grpc"))]
use crate::observability::metrics::{MetricRouteTemplateLabel, UNKNOWN_ROUTE_TEMPLATE};

#[test]
fn route_template_validation_rejects_invalid_values() {
    assert_eq!(
        RouteTemplate::new(""),
        Err(MetricLabelError::EmptyRouteTemplate)
    );
    assert_eq!(
        RouteTemplate::new("readyz"),
        Err(MetricLabelError::RouteTemplateMustStartWithSlash)
    );
    assert_eq!(
        RouteTemplate::new("/readyz?verbose=true"),
        Err(MetricLabelError::RouteTemplateMustNotContainQueryString)
    );
    assert_eq!(
        RouteTemplate::new("/https://api.reallyme.net/readyz"),
        Err(MetricLabelError::RouteTemplateMustNotContainUrlScheme)
    );
    assert_eq!(
        RouteTemplate::new("/ready z"),
        Err(MetricLabelError::RouteTemplateMustNotContainWhitespace)
    );
    assert_eq!(
        RouteTemplate::new("/readyz").map(|route| route.as_str()),
        Ok("/readyz")
    );
}

#[test]
fn status_class_maps_to_expected_label() {
    assert_eq!(HttpStatusClass::from_status_code(200).as_str(), "2xx");
    assert_eq!(HttpStatusClass::from_status_code(503).as_str(), "5xx");
}

#[cfg(all(feature = "http", feature = "tonic-grpc"))]
#[test]
fn retained_route_template_labels_reuse_interned_storage() {
    let first = MetricRouteTemplateLabel::from_runtime_route_template("/v1/accounts/:account_id");
    let second = MetricRouteTemplateLabel::from_runtime_route_template("/v1/accounts/:account_id");

    let first_label = first.clone_shared();
    let second_label = second.clone_shared();
    let first_value: &str = first_label.as_ref();
    let second_value: &str = second_label.as_ref();

    assert_eq!(first_value, "/v1/accounts/:account_id");
    assert!(std::ptr::eq(first_value, second_value));
}

#[cfg(all(feature = "http", feature = "tonic-grpc"))]
#[test]
fn retained_route_template_labels_reject_raw_path_shapes() {
    let query_path = MetricRouteTemplateLabel::from_runtime_route_template("/readyz?verbose=true");
    let url_path =
        MetricRouteTemplateLabel::from_runtime_route_template("/https://api.reallyme.net/readyz");
    let whitespace_path = MetricRouteTemplateLabel::from_runtime_route_template("/ready z");

    assert_eq!(query_path.clone_shared().as_ref(), UNKNOWN_ROUTE_TEMPLATE);
    assert_eq!(url_path.clone_shared().as_ref(), UNKNOWN_ROUTE_TEMPLATE);
    assert_eq!(
        whitespace_path.clone_shared().as_ref(),
        UNKNOWN_ROUTE_TEMPLATE
    );
}
