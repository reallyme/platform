// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use tonic::Code;

use super::{GrpcStatusCode, StaticGrpcStatus, ToGrpcStatus};
use crate::grpc::{
    GrpcDeadlineConfigField, GrpcDeadlineError, GrpcDeadlineErrorReason, GrpcMetadataError,
    GrpcMetadataErrorReason, GrpcMetadataField,
};

#[test]
fn maps_deadline_exceeded_reason() {
    let status_code: Code = GrpcStatusCode::DeadlineExceeded.into();

    assert_eq!(status_code, Code::DeadlineExceeded);
}

#[test]
fn converts_static_status_to_tonic_status() {
    let status = StaticGrpcStatus::new(GrpcStatusCode::AlreadyExists, "resource already exists")
        .to_grpc_status();

    assert_eq!(status.code(), Code::AlreadyExists);
    assert_eq!(status.message(), "resource already exists");
}

#[test]
fn metadata_errors_map_to_invalid_argument() {
    let status = GrpcMetadataError::InvalidMetadata {
        field: GrpcMetadataField::RequestId,
        reason: GrpcMetadataErrorReason::InvalidUuid,
    }
    .to_grpc_status();

    assert_eq!(status.code(), Code::InvalidArgument);
    assert_eq!(status.message(), "invalid grpc metadata");
}

#[test]
fn invalid_timeout_configuration_maps_to_failed_precondition() {
    let status = GrpcDeadlineError::InvalidTimeoutConfiguration {
        field: GrpcDeadlineConfigField::MaximumTimeout,
        reason: GrpcDeadlineErrorReason::MustBeGreaterThanZero,
    }
    .to_grpc_status();

    assert_eq!(status.code(), Code::FailedPrecondition);
    assert_eq!(status.message(), "grpc timeout configuration is invalid");
}
