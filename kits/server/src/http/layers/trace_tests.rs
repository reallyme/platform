// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use crate::config::{
    HttpRequestLogField, HttpRequestLogFields, HttpRequestLogMode, HttpRequestLoggingConfig,
};
use crate::observability::{HttpMethodLabel, HttpStatusClass};

use super::should_log_request_completion;

fn logging(mode: HttpRequestLogMode, sample_rate: Option<f64>) -> HttpRequestLoggingConfig {
    HttpRequestLoggingConfig::new(mode, sample_rate, None).expect("valid logging config")
}

#[test]
fn logging_config_retains_selected_structured_fields() {
    let config = HttpRequestLoggingConfig::new(HttpRequestLogMode::All, None, None)
        .expect("valid logging config")
        .with_fields(
            HttpRequestLogFields::defaults()
                .with_field(HttpRequestLogField::ListenerName)
                .with_field(HttpRequestLogField::ExternalOrigin),
        );

    assert!(config.fields().listener_name());
    assert!(config.fields().external_origin());
    assert!(!config.fields().normalized_client_ip());
}

#[test]
fn disabled_mode_does_not_log_successful_requests() {
    assert!(!should_log_request_completion(
        logging(HttpRequestLogMode::Disabled, None),
        HttpStatusClass::Success,
        &HttpMethodLabel::Get,
        "/hello",
        Duration::from_millis(2),
        false,
    ));
}

#[test]
fn errors_only_logs_error_responses() {
    assert!(should_log_request_completion(
        logging(HttpRequestLogMode::ErrorsOnly, None),
        HttpStatusClass::ServerError,
        &HttpMethodLabel::Get,
        "/hello",
        Duration::from_millis(2),
        false,
    ));
    assert!(should_log_request_completion(
        logging(HttpRequestLogMode::ErrorsOnly, None),
        HttpStatusClass::ClientError,
        &HttpMethodLabel::Get,
        "/hello",
        Duration::from_millis(2),
        false,
    ));
}

#[test]
fn all_mode_logs_every_request() {
    assert!(should_log_request_completion(
        logging(HttpRequestLogMode::All, None),
        HttpStatusClass::Success,
        &HttpMethodLabel::Get,
        "/hello",
        Duration::from_millis(2),
        false,
    ));
}
