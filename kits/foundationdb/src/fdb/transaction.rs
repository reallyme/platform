// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transaction policy helpers.

use std::time::Duration;
use thiserror::Error;

/// Transaction policy construction that failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TransactionPolicyError {
    /// Retry budget must be within safe operating range.
    #[error("retry limit must be between {min} and {max}")]
    InvalidRetryLimit {
        /// The minimum allowed retry attempts.
        min: u32,
        /// The maximum allowed retry attempts.
        max: u32,
        /// Caller-provided value.
        value: u32,
    },
    /// Timeout must be in the configured policy window.
    #[error("transaction timeout must be between {min_ms}ms and {max_ms}ms")]
    InvalidTimeout {
        /// Minimum timeout in milliseconds.
        min_ms: u64,
        /// Maximum timeout in milliseconds.
        max_ms: u64,
        /// Caller-provided timeout in milliseconds.
        value_ms: u64,
    },
}

const MIN_RETRY_LIMIT: u32 = 1;
const MAX_RETRY_LIMIT: u32 = 16;
const MIN_TIMEOUT_MS: u64 = 1;
const MAX_TIMEOUT_MS: u64 = 60_000;

/// Typed read-optimized transaction behavior.
///
/// Reads usually benefit from smaller deadlines and lower retry pressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadTxnPolicy {
    retry_limit: u32,
    time_out: Duration,
}

/// Typed mutation-optimized transaction behavior.
///
/// Write paths should keep the higher timeout and retries needed for fan-out
/// and conflict chains on contended keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteTxnPolicy {
    retry_limit: u32,
    time_out: Duration,
}

impl ReadTxnPolicy {
    /// Builds an explicit read policy with validated parameters.
    pub fn try_new(retry_limit: u32, time_out: Duration) -> Result<Self, TransactionPolicyError> {
        validate_policy_limits(retry_limit, time_out)?;
        Ok(Self {
            retry_limit,
            time_out,
        })
    }

    /// Default policy for bounded, fast-failing reads.
    pub const fn default() -> Self {
        Self {
            retry_limit: 2,
            time_out: Duration::from_millis(500),
        }
    }

    /// Returns the maximum total attempts, including the initial attempt.
    pub const fn retry_limit(&self) -> u32 {
        self.retry_limit
    }

    /// Returns the deadline for the complete transaction, including retries.
    pub const fn time_out(&self) -> Duration {
        self.time_out
    }

    /// Converts this policy to FoundationDB transact options.
    pub const fn to_transact_option(self) -> foundationdb::TransactOption {
        foundationdb::TransactOption {
            retry_limit: Some(self.retry_limit),
            time_out: Some(self.time_out),
            is_idempotent: true,
        }
    }
}

impl WriteTxnPolicy {
    /// Builds an explicit write policy with validated parameters.
    pub fn try_new(retry_limit: u32, time_out: Duration) -> Result<Self, TransactionPolicyError> {
        validate_policy_limits(retry_limit, time_out)?;
        Ok(Self {
            retry_limit,
            time_out,
        })
    }

    /// Default policy for mutations and commits.
    pub const fn default() -> Self {
        Self {
            retry_limit: 3,
            time_out: Duration::from_secs(5),
        }
    }

    /// Returns the maximum total attempts, including the initial attempt.
    pub const fn retry_limit(&self) -> u32 {
        self.retry_limit
    }

    /// Returns the deadline for the complete transaction, including retries.
    pub const fn time_out(&self) -> Duration {
        self.time_out
    }

    /// Converts this policy to FoundationDB transact options.
    pub const fn to_transact_option(self) -> foundationdb::TransactOption {
        foundationdb::TransactOption {
            retry_limit: Some(self.retry_limit),
            time_out: Some(self.time_out),
            is_idempotent: false,
        }
    }
}

/// Builds the default transaction policy used by idempotent read paths.
///
/// The `idempotent` flag is set so FoundationDB may safely retry retryable errors
/// without risking duplicate side effects.
pub fn idempotent_read_option() -> foundationdb::TransactOption {
    ReadTxnPolicy::default().to_transact_option()
}

/// Builds the default transaction policy used by mutation paths.
///
/// The write policy is non-idempotent so a commit that reports
/// `maybe_committed` is surfaced as non-retryable and can be handled only under
/// caller-owned at-most-once invariants.
///
/// Cancellation invariants:
/// - Write transactions must tolerate `TransactError` outcomes that arrive after
///   the caller canceled the future and treat a canceled path as potentially
///   uncertain if a commit may have reached the cluster.
/// - If an operation can be retried safely by caller logic, it should use
///   [`idempotent_read_option`] instead.
pub fn mutation_option() -> foundationdb::TransactOption {
    WriteTxnPolicy::default().to_transact_option()
}

fn validate_policy_limits(
    retry_limit: u32,
    time_out: Duration,
) -> Result<(), TransactionPolicyError> {
    if !(MIN_RETRY_LIMIT..=MAX_RETRY_LIMIT).contains(&retry_limit) {
        return Err(TransactionPolicyError::InvalidRetryLimit {
            min: MIN_RETRY_LIMIT,
            max: MAX_RETRY_LIMIT,
            value: retry_limit,
        });
    }

    let time_out_ms = u64::try_from(time_out.as_millis()).map_err(|_| {
        TransactionPolicyError::InvalidTimeout {
            min_ms: MIN_TIMEOUT_MS,
            max_ms: MAX_TIMEOUT_MS,
            value_ms: MAX_TIMEOUT_MS.saturating_add(1),
        }
    })?;

    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&time_out_ms) {
        return Err(TransactionPolicyError::InvalidTimeout {
            min_ms: MIN_TIMEOUT_MS,
            max_ms: MAX_TIMEOUT_MS,
            value_ms: time_out_ms,
        });
    }

    Ok(())
}

#[cfg(test)]
#[path = "transaction_tests.rs"]
mod tests;
