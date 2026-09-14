// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

use crate::config::ServiceEnvironment;

/// gRPC metadata fields controlled by the server kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcMetadataField {
    /// The propagated request identifier.
    RequestId,
    /// The propagated trace identifier.
    TraceId,
}

/// Why gRPC metadata could not be validated or emitted safely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcMetadataErrorReason {
    /// The metadata value is not valid visible ASCII.
    InvalidAscii,
    /// The metadata value is not a valid UUID.
    InvalidUuid,
    /// The metadata value could not be emitted as gRPC metadata.
    InvalidMetadataValue,
}

/// Typed failures for gRPC metadata handling.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GrpcMetadataError {
    /// The metadata field was invalid.
    #[error("grpc metadata field is invalid")]
    InvalidMetadata {
        /// The invalid field.
        field: GrpcMetadataField,
        /// Why the field was invalid.
        reason: GrpcMetadataErrorReason,
    },
}

/// Timeout configuration fields controlled by the server kit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcDeadlineConfigField {
    /// The maximum timeout applied by the server.
    MaximumTimeout,
}

/// Timeout-related metadata fields controlled by the gRPC protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcDeadlineMetadataField {
    /// The `grpc-timeout` metadata field.
    GrpcTimeout,
}

/// Why a timeout configuration or metadata value was invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrpcDeadlineErrorReason {
    /// The timeout must be greater than zero.
    MustBeGreaterThanZero,
    /// The metadata field is empty.
    Empty,
    /// The metadata field is not valid visible ASCII.
    InvalidAscii,
    /// The metadata field did not contain a numeric value.
    MissingValue,
    /// The numeric portion could not be parsed.
    InvalidInteger,
    /// The metadata unit suffix is not supported by gRPC.
    InvalidUnit,
    /// The numeric portion exceeded the supported eight-digit format.
    TooManyDigits,
    /// The value overflowed the supported duration range.
    Overflow,
}

/// Typed deadline and timeout failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GrpcDeadlineError {
    /// The configured server-side timeout was invalid.
    #[error("grpc timeout configuration is invalid")]
    InvalidTimeoutConfiguration {
        /// The invalid configuration field.
        field: GrpcDeadlineConfigField,
        /// Why the field was invalid.
        reason: GrpcDeadlineErrorReason,
    },
    /// The inbound `grpc-timeout` metadata was invalid.
    #[error("grpc timeout metadata is invalid")]
    InvalidTimeoutMetadata {
        /// The invalid metadata field.
        field: GrpcDeadlineMetadataField,
        /// Why the field was invalid.
        reason: GrpcDeadlineErrorReason,
    },
    /// The request exceeded its effective deadline.
    #[error("grpc request deadline was exceeded")]
    DeadlineExceeded,
}

/// Typed reflection-toggle failures.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum GrpcReflectionError {
    /// Reflection was requested in an environment where the selected mode does
    /// not permit it.
    #[error("grpc reflection is not enabled for this environment")]
    ReflectionNotAllowedInEnvironment {
        /// The environment in which reflection was requested.
        service_environment: ServiceEnvironment,
    },
}
