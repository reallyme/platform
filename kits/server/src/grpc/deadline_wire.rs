// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{num::NonZeroU32, time::Duration};

// Eight digits followed by H is the largest representable wire duration.
pub(super) const MAX_GRPC_TIMEOUT: Duration = Duration::from_secs(359_999_996_400);
const MAX_WIRE_BYTES: usize = 9;

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
    /// More than one timeout was supplied for the call.
    DuplicateTimeout,
}

pub(super) fn parse_grpc_timeout_value(value: &[u8]) -> Result<Duration, GrpcDeadlineErrorReason> {
    let (digits, unit) = parse_timeout_shape(value)?;
    let amount = digits.iter().try_fold(0_u32, append_digit)?;
    let amount = NonZeroU32::new(amount).ok_or(GrpcDeadlineErrorReason::MustBeGreaterThanZero)?;
    unit.duration(amount)
}

pub(super) fn parse_timeout_shape(
    value: &[u8],
) -> Result<(&[u8], TimeoutUnit), GrpcDeadlineErrorReason> {
    use self::GrpcDeadlineErrorReason as Reason;

    let (unit, digits) = value.split_last().ok_or(Reason::Empty)?;
    if digits.is_empty() {
        return Err(Reason::MissingValue);
    }
    if value.len() > MAX_WIRE_BYTES {
        return Err(Reason::TooManyDigits);
    }
    if value.iter().any(|byte| !byte.is_ascii()) {
        return Err(Reason::InvalidAscii);
    }

    if !digits.iter().all(u8::is_ascii_digit) {
        return Err(Reason::InvalidInteger);
    }

    let unit = match unit {
        b'H' => TimeoutUnit::Hours,
        b'M' => TimeoutUnit::Minutes,
        b'S' => TimeoutUnit::Seconds,
        b'm' => TimeoutUnit::Millis,
        b'u' => TimeoutUnit::Micros,
        b'n' => TimeoutUnit::Nanos,
        _ => return Err(Reason::InvalidUnit),
    };
    if digits.iter().all(|digit| *digit == b'0') {
        return Err(Reason::MustBeGreaterThanZero);
    }
    Ok((digits, unit))
}

pub(super) fn append_digit(amount: u32, digit: &u8) -> Result<u32, GrpcDeadlineErrorReason> {
    let invalid = GrpcDeadlineErrorReason::InvalidInteger;
    let value = digit.checked_sub(b'0').ok_or(invalid)?;
    amount
        .checked_mul(10)
        .and_then(|amount| amount.checked_add(u32::from(value)))
        .ok_or(invalid)
}

pub(super) enum TimeoutUnit {
    Hours,
    Minutes,
    Seconds,
    Millis,
    Micros,
    Nanos,
}

impl TimeoutUnit {
    pub(super) fn duration(self, amount: NonZeroU32) -> Result<Duration, GrpcDeadlineErrorReason> {
        let amount = u64::from(amount.get());
        let overflow = GrpcDeadlineErrorReason::Overflow;
        Ok(match self {
            Self::Hours => Duration::from_secs(amount.checked_mul(3_600).ok_or(overflow)?),
            Self::Minutes => Duration::from_secs(amount.checked_mul(60).ok_or(overflow)?),
            Self::Seconds => Duration::from_secs(amount),
            Self::Millis => Duration::from_millis(amount),
            Self::Micros => Duration::from_micros(amount),
            Self::Nanos => Duration::from_nanos(amount),
        })
    }
}
