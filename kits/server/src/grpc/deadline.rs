// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::time::Duration;

use tonic::Request;

use super::error::{
    GrpcDeadlineConfigField, GrpcDeadlineError, GrpcDeadlineErrorReason, GrpcDeadlineMetadataField,
};

/// Standard gRPC timeout metadata key.
pub const GRPC_TIMEOUT_METADATA_KEY: &str = "grpc-timeout";

// gRPC permits at most eight digits; hours are its largest timeout unit.
// Keep construction within tonic's serialization range to prevent a panic.
const MAX_GRPC_TIMEOUT: Duration = Duration::from_secs(359_999_996_400);

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
    request
        .metadata()
        .get(GRPC_TIMEOUT_METADATA_KEY)
        .map(parse_timeout_metadata)
        .transpose()
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

fn parse_timeout_metadata(
    value: &tonic::metadata::MetadataValue<tonic::metadata::Ascii>,
) -> Result<GrpcTimeout, GrpcDeadlineError> {
    let value = value
        .to_str()
        .map_err(|_| GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::InvalidAscii,
        })?;

    let duration = parse_grpc_timeout_value(value)?;
    GrpcTimeout::new(duration)
}

fn parse_grpc_timeout_value(value: &str) -> Result<Duration, GrpcDeadlineError> {
    if value.is_empty() {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::Empty,
        });
    }

    if value.len() == 1 {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::MissingValue,
        });
    }

    if value.len() > 9 {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::TooManyDigits,
        });
    }

    if !value.is_ascii() {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::InvalidAscii,
        });
    }
    let unit_offset =
        value
            .len()
            .checked_sub(1)
            .ok_or(GrpcDeadlineError::InvalidTimeoutMetadata {
                field: GrpcDeadlineMetadataField::GrpcTimeout,
                reason: GrpcDeadlineErrorReason::MissingValue,
            })?;
    let (digits, unit) = value.split_at(unit_offset);
    if digits.is_empty() {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::MissingValue,
        });
    }

    if !digits.bytes().all(|digit| digit.is_ascii_digit()) {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::InvalidInteger,
        });
    }

    let amount = digits
        .parse::<u64>()
        .map_err(|_| GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::InvalidInteger,
        })?;

    let duration = match unit {
        "H" => duration_from_hours(amount),
        "M" => duration_from_minutes(amount),
        "S" => duration_from_seconds(amount),
        "m" => Some(Duration::from_millis(amount)),
        "u" => Some(Duration::from_micros(amount)),
        "n" => Some(Duration::from_nanos(amount)),
        _ => {
            return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
                field: GrpcDeadlineMetadataField::GrpcTimeout,
                reason: GrpcDeadlineErrorReason::InvalidUnit,
            });
        }
    }
    .ok_or(GrpcDeadlineError::InvalidTimeoutMetadata {
        field: GrpcDeadlineMetadataField::GrpcTimeout,
        reason: GrpcDeadlineErrorReason::Overflow,
    })?;

    if duration.is_zero() {
        return Err(GrpcDeadlineError::InvalidTimeoutMetadata {
            field: GrpcDeadlineMetadataField::GrpcTimeout,
            reason: GrpcDeadlineErrorReason::MustBeGreaterThanZero,
        });
    }

    Ok(duration)
}

fn duration_from_hours(value: u64) -> Option<Duration> {
    value
        .checked_mul(60)?
        .checked_mul(60)
        .map(Duration::from_secs)
}

fn duration_from_minutes(value: u64) -> Option<Duration> {
    value.checked_mul(60).map(Duration::from_secs)
}

fn duration_from_seconds(value: u64) -> Option<Duration> {
    Some(Duration::from_secs(value))
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
