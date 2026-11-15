// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::time::Duration;

use tonic::Request;

use super::deadline_wire::{MAX_GRPC_TIMEOUT, parse_grpc_timeout_value};
use super::error::{
    GrpcDeadlineConfigField, GrpcDeadlineError, GrpcDeadlineErrorReason, GrpcDeadlineMetadataField,
};

/// Standard gRPC timeout metadata key.
pub const GRPC_TIMEOUT_METADATA_KEY: &str = "grpc-timeout";

/// Validated gRPC timeout used for caller deadlines and server-side maximums.
///
/// This type intentionally rejects zero values so services fail closed rather
/// than accidentally running unbounded requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GrpcTimeout(Duration);

impl GrpcTimeout {
    /// Creates a validated gRPC timeout.
    pub fn new(value: Duration) -> Result<Self, GrpcDeadlineError> {
        if value.is_zero() {
            return Err(GrpcDeadlineError::InvalidTimeoutConfiguration {
                field: GrpcDeadlineConfigField::MaximumTimeout,
                reason: GrpcDeadlineErrorReason::MustBeGreaterThanZero,
            });
        }

        if value > MAX_GRPC_TIMEOUT {
            return Err(GrpcDeadlineError::InvalidTimeoutConfiguration {
                field: GrpcDeadlineConfigField::MaximumTimeout,
                reason: GrpcDeadlineErrorReason::Overflow,
            });
        }

        Ok(Self(value))
    }

    /// Returns the underlying duration.
    pub fn as_duration(self) -> Duration {
        self.0
    }
}

/// Applies a client-side timeout to an outbound gRPC request.
pub fn apply_client_timeout<T>(request: &mut Request<T>, timeout: GrpcTimeout) {
    request.set_timeout(timeout.as_duration());
}

/// Parses the caller-provided gRPC timeout from inbound metadata.
pub fn caller_timeout<T>(request: &Request<T>) -> Result<Option<GrpcTimeout>, GrpcDeadlineError> {
    let mut values = request.metadata().get_all(GRPC_TIMEOUT_METADATA_KEY).iter();
    let Some(value) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(invalid_timeout_metadata(
            GrpcDeadlineErrorReason::DuplicateTimeout,
        ));
    }
    let text = value
        .to_str()
        .map_err(|_| invalid_timeout_metadata(GrpcDeadlineErrorReason::InvalidAscii))?;
    parse_timeout(text.as_bytes()).map(Some)
}

/// Returns the effective server-side timeout for the request.
///
/// If the caller supplied a timeout, the effective timeout is the smaller of
/// the caller's deadline and the service's configured maximum.
pub fn effective_timeout<T>(
    request: &Request<T>,
    maximum_timeout: GrpcTimeout,
) -> Result<GrpcTimeout, GrpcDeadlineError> {
    match caller_timeout(request)? {
        Some(caller_timeout) => Ok(std::cmp::min(caller_timeout, maximum_timeout)),
        None => Ok(maximum_timeout),
    }
}

/// Runs a future with a validated timeout budget.
///
/// Dropping a timed-out future is only safe if the future itself is
/// cancellation-safe. Callers remain responsible for choosing operations with
/// correct cancellation behavior.
pub async fn run_with_timeout<F, T>(timeout: GrpcTimeout, future: F) -> Result<T, GrpcDeadlineError>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(timeout.as_duration(), future)
        .await
        .map_err(|_| GrpcDeadlineError::DeadlineExceeded)
}

pub(super) fn parse_timeout(bytes: &[u8]) -> Result<GrpcTimeout, GrpcDeadlineError> {
    let duration = parse_grpc_timeout_value(bytes).map_err(invalid_timeout_metadata)?;
    GrpcTimeout::new(duration)
}

fn invalid_timeout_metadata(reason: GrpcDeadlineErrorReason) -> GrpcDeadlineError {
    GrpcDeadlineError::InvalidTimeoutMetadata {
        field: GrpcDeadlineMetadataField::GrpcTimeout,
        reason,
    }
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
