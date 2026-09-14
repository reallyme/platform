// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::HeaderValue;

use super::header_value_is_json_content_type;

#[test]
fn json_content_type_detection_is_case_insensitive_and_parameter_safe() {
    assert!(header_value_is_json_content_type(
        &HeaderValue::from_static("application/json")
    ));
    assert!(header_value_is_json_content_type(
        &HeaderValue::from_static("Application/Json; Charset=UTF-8")
    ));
    assert!(header_value_is_json_content_type(
        &HeaderValue::from_static("application/problem+json")
    ));
    assert!(!header_value_is_json_content_type(
        &HeaderValue::from_static("text/plain")
    ));
}
