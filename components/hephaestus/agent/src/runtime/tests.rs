// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::HephaestusAgentErrorReason;
use reallyme_hephaestus_contract::generated::proto::reallyme::domain::v1 as domain_pb;
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as hephaestus_pb;
use std::cell::RefCell;
use std::time::Duration;

#[test]
fn first_registration_retry_delay_doubles_until_cap() {
    let first = super::next_registration_retry_delay(
        super::FIRST_REGISTRATION_INITIAL_RETRY_DELAY,
        super::FIRST_REGISTRATION_MAX_RETRY_DELAY,
    );
    let second =
        super::next_registration_retry_delay(first, super::FIRST_REGISTRATION_MAX_RETRY_DELAY);
    let capped = super::next_registration_retry_delay(
        super::FIRST_REGISTRATION_MAX_RETRY_DELAY,
        super::FIRST_REGISTRATION_MAX_RETRY_DELAY,
    );

    assert_eq!(first.as_secs(), 4);
    assert_eq!(second.as_secs(), 8);
    assert_eq!(capped, super::FIRST_REGISTRATION_MAX_RETRY_DELAY);
}

#[tokio::test]
async fn first_registration_retry_retries_transient_connect_failures_until_success() {
    let mut attempts = 0u32;
    let delays = RefCell::new(Vec::new());

    let response = super::register_agent_with_retry(
        || {
            attempts += 1;
            let result = if attempts < 3 {
                Err(crate::error::HephaestusAgentError::new(
                    HephaestusAgentErrorReason::ConnectFailed,
                ))
            } else {
                Ok(accepted_registration_response())
            };
            std::future::ready(result)
        },
        Duration::from_secs(2),
        Duration::from_secs(60),
        |delay| {
            delays.borrow_mut().push(delay);
            std::future::ready(())
        },
    )
    .await
    .expect("transient registration failures should retry");

    assert_eq!(attempts, 3);
    assert_eq!(
        delays.into_inner(),
        vec![Duration::from_secs(2), Duration::from_secs(4)]
    );
    assert_eq!(response.runtime_token, "runtime-token-1234");
}

#[tokio::test]
async fn first_registration_retry_fails_closed_for_non_transient_errors() {
    let mut attempts = 0u32;
    let delays = RefCell::new(Vec::new());

    let result = super::register_agent_with_retry(
        || {
            attempts += 1;
            std::future::ready(Err(crate::error::HephaestusAgentError::new(
                HephaestusAgentErrorReason::RegistrationRejected,
            )))
        },
        Duration::from_secs(2),
        Duration::from_secs(60),
        |delay| {
            delays.borrow_mut().push(delay);
            std::future::ready(())
        },
    )
    .await;

    assert_eq!(
        result.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::RegistrationRejected)
    );
    assert_eq!(attempts, 1);
    assert!(delays.into_inner().is_empty());
}

#[test]
fn parse_runtime_token_from_registration_response_rejects_too_short_tokens() {
    let result = super::runtime_token_from_registration_response("short");

    assert_eq!(
        result.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::RuntimeTokenUnavailable)
    );
}

fn accepted_registration_response() -> hephaestus_pb::RegisterAgentResponse {
    hephaestus_pb::RegisterAgentResponse {
        result: buffa::EnumValue::from(
            domain_pb::HephaestusAgentBootReportResult::HEPHAESTUS_AGENT_BOOT_REPORT_RESULT_ACCEPTED
                as i32,
        ),
        actual_state: Default::default(),
        runtime_token: "runtime-token-1234".to_owned(),
        desired_generation: 7,
        poll_after_seconds: 2,
        __buffa_unknown_fields: Default::default(),
    }
}

#[test]
fn parse_runtime_token_from_registration_response_accepts_minimum_length_tokens() {
    let result = super::runtime_token_from_registration_response("runtime-token-123");

    assert!(result.is_ok());
}

#[test]
fn parse_runtime_token_from_registration_response_rejects_too_long_tokens() {
    let token = "x".repeat(crate::state::MAX_RUNTIME_TOKEN_BYTES + 1);
    let result = super::runtime_token_from_registration_response(token.as_str());

    assert_eq!(
        result.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::RuntimeTokenUnavailable)
    );
}
