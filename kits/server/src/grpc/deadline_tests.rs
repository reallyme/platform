// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use tonic::Request;

use super::{
    GRPC_TIMEOUT_METADATA_KEY, GrpcTimeout, apply_client_timeout, caller_timeout,
    effective_timeout, run_with_timeout,
};
use crate::grpc::{
    GrpcDeadlineConfigField, GrpcDeadlineError, GrpcDeadlineErrorReason, GrpcDeadlineMetadataField,
};

#[test]
fn grpc_timeout_rejects_zero_values() {
    let timeout = GrpcTimeout::new(Duration::ZERO);

    assert_eq!(
        timeout,
        Err(GrpcDeadlineError::InvalidTimeoutConfiguration {
            field: GrpcDeadlineConfigField::MaximumTimeout,
            reason: GrpcDeadlineErrorReason::MustBeGreaterThanZero,
        })
    );
}

#[test]
fn timeout_rejects_unserializable_durations() {
    assert!(GrpcTimeout::new(Duration::MAX).is_err());
    assert!(GrpcTimeout::new(super::MAX_GRPC_TIMEOUT + Duration::from_nanos(1)).is_err());
    let timeout = GrpcTimeout::new(super::MAX_GRPC_TIMEOUT).expect("protocol maximum");
    let mut request = Request::new(());
    apply_client_timeout(&mut request, timeout);
    assert_eq!(caller_timeout(&request), Ok(Some(timeout)));
}

#[test]
fn timeout_parser_rejects_signed_and_non_ascii_values() {
    for value in ["+1S", "-1S", "1é", "éS"] {
        assert!(super::parse_grpc_timeout_value(value).is_err());
    }
}

#[test]
fn caller_timeout_parses_valid_metadata() {
    let mut request = Request::new(());
    request.metadata_mut().insert(
        GRPC_TIMEOUT_METADATA_KEY,
        "2500m"
            .parse()
            .expect("static timeout metadata should parse"),
    );

    let timeout = caller_timeout(&request).expect("timeout metadata should parse");

    assert_eq!(
        timeout,
        Some(GrpcTimeout::new(Duration::from_millis(2500)).expect("fixture should be valid"))
    );
}

#[test]
fn caller_timeout_rejects_invalid_metadata() {
    let mut request = Request::new(());
    request.metadata_mut().insert(
        GRPC_TIMEOUT_METADATA_KEY,
        "abc"
            .parse()
            .expect("static invalid timeout should still be metadata"),
    );

    let timeout = caller_timeout(&request);

    assert_eq!(
        timeout,
        Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::InvalidInteger,
        })
    );
}

#[test]
fn caller_timeout_rejects_too_many_digits() {
    let mut request = Request::new(());
    request.metadata_mut().insert(
        GRPC_TIMEOUT_METADATA_KEY,
        "123456789S"
            .parse()
            .expect("static oversized timeout should still be metadata"),
    );

    let timeout = caller_timeout(&request);

    assert_eq!(
        timeout,
        Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::TooManyDigits,
        })
    );
}

#[test]
fn effective_timeout_uses_smaller_caller_deadline() {
    let maximum_timeout =
        GrpcTimeout::new(Duration::from_secs(5)).expect("fixture should be valid");
    let mut request = Request::new(());
    request.metadata_mut().insert(
        GRPC_TIMEOUT_METADATA_KEY,
        "2S".parse().expect("static timeout metadata should parse"),
    );

    let timeout =
        effective_timeout(&request, maximum_timeout).expect("effective timeout should parse");

    assert_eq!(timeout.as_duration(), Duration::from_secs(2));
}

#[test]
fn client_timeout_helper_writes_grpc_timeout_metadata() {
    let timeout = GrpcTimeout::new(Duration::from_secs(30)).expect("fixture should be valid");
    let mut request = Request::new(());

    apply_client_timeout(&mut request, timeout);

    assert_eq!(
        request.metadata().get(GRPC_TIMEOUT_METADATA_KEY),
        Some(
            &"30000000u"
                .parse()
                .expect("tonic-compatible timeout metadata should parse")
        )
    );
}

#[tokio::test]
async fn run_with_timeout_maps_elapsed_to_deadline_exceeded() {
    let timeout = GrpcTimeout::new(Duration::from_millis(10)).expect("fixture should be valid");

    let result = run_with_timeout(timeout, async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        42_u8
    })
    .await;

    assert_eq!(result, Err(GrpcDeadlineError::DeadlineExceeded));
}
