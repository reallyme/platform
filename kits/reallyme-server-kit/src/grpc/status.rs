// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use tonic::{Code, Status};

use super::error::{GrpcDeadlineError, GrpcMetadataError, GrpcReflectionError};

/// Stable gRPC status codes emitted by infrastructure and service adapters.
///
/// These variants intentionally model transport semantics only. They are more
/// precise than a generic "conflict" bucket so callers can distinguish
/// `AlreadyExists`, `FailedPrecondition`, `Aborted`, and `DeadlineExceeded`
/// according to the actual failure mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcStatusCode {
    /// The caller supplied invalid input.
    InvalidArgument,
    /// The caller could not be authenticated.
    Unauthenticated,
    /// The caller is authenticated but not authorized.
    PermissionDenied,
    /// The requested resource does not exist.
    NotFound,
    /// The resource already exists.
    AlreadyExists,
    /// The system state does not satisfy a required precondition.
    FailedPrecondition,
    /// The operation was aborted due to concurrency or sequencing issues.
    Aborted,
    /// Capacity or quota has been exceeded.
    ResourceExhausted,
    /// The request exceeded its deadline.
    DeadlineExceeded,
    /// The downstream service is temporarily unavailable.
    Unavailable,
    /// An unexpected internal failure occurred.
    Internal,
}

impl From<GrpcStatusCode> for Code {
    fn from(value: GrpcStatusCode) -> Self {
        match value {
            GrpcStatusCode::InvalidArgument => Code::InvalidArgument,
            GrpcStatusCode::Unauthenticated => Code::Unauthenticated,
            GrpcStatusCode::PermissionDenied => Code::PermissionDenied,
            GrpcStatusCode::NotFound => Code::NotFound,
            GrpcStatusCode::AlreadyExists => Code::AlreadyExists,
            GrpcStatusCode::FailedPrecondition => Code::FailedPrecondition,
            GrpcStatusCode::Aborted => Code::Aborted,
            GrpcStatusCode::ResourceExhausted => Code::ResourceExhausted,
            GrpcStatusCode::DeadlineExceeded => Code::DeadlineExceeded,
            GrpcStatusCode::Unavailable => Code::Unavailable,
            GrpcStatusCode::Internal => Code::Internal,
        }
    }
}

/// Converts a type into a gRPC status.
///
/// Service-specific errors should implement this trait when they need full
/// control over both transport code and safe public error text. Implementations
/// must not leak internal diagnostics, secret material, downstream names, or
/// other non-public details through the returned status message.
pub trait ToGrpcStatus {
    /// Converts the value into a gRPC status.
    fn to_grpc_status(&self) -> Status;
}

/// Static gRPC status helper for transport-safe code and message pairs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticGrpcStatus {
    code: GrpcStatusCode,
    message: &'static str,
}

impl StaticGrpcStatus {
    /// Creates a static gRPC status helper.
    pub fn new(code: GrpcStatusCode, message: &'static str) -> Self {
        Self { code, message }
    }

    /// Returns the configured status code.
    pub fn code(&self) -> GrpcStatusCode {
        self.code
    }

    /// Returns the configured public message.
    pub fn message(&self) -> &'static str {
        self.message
    }
}

impl ToGrpcStatus for StaticGrpcStatus {
    fn to_grpc_status(&self) -> Status {
        Status::new(self.code.into(), self.message)
    }
}

impl ToGrpcStatus for GrpcMetadataError {
    fn to_grpc_status(&self) -> Status {
        StaticGrpcStatus::new(GrpcStatusCode::InvalidArgument, "invalid grpc metadata")
            .to_grpc_status()
    }
}

impl ToGrpcStatus for GrpcDeadlineError {
    fn to_grpc_status(&self) -> Status {
        match self {
            GrpcDeadlineError::InvalidTimeoutConfiguration { .. } => StaticGrpcStatus::new(
                GrpcStatusCode::FailedPrecondition,
                "grpc timeout configuration is invalid",
            )
            .to_grpc_status(),
            GrpcDeadlineError::InvalidTimeoutMetadata { .. } => StaticGrpcStatus::new(
                GrpcStatusCode::InvalidArgument,
                "invalid grpc timeout metadata",
            )
            .to_grpc_status(),
            GrpcDeadlineError::DeadlineExceeded => {
                StaticGrpcStatus::new(GrpcStatusCode::DeadlineExceeded, "deadline exceeded")
                    .to_grpc_status()
            }
        }
    }
}

impl ToGrpcStatus for GrpcReflectionError {
    fn to_grpc_status(&self) -> Status {
        StaticGrpcStatus::new(
            GrpcStatusCode::FailedPrecondition,
            "grpc reflection is not enabled for this environment",
        )
        .to_grpc_status()
    }
}

#[cfg(test)]
mod tests {
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
        let status =
            StaticGrpcStatus::new(GrpcStatusCode::AlreadyExists, "resource already exists")
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
}
