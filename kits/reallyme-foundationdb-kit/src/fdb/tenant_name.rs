// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated, allocation-free FoundationDB tenant names.

use std::fmt;
use std::fmt::Write as _;

use thiserror::Error;

/// Maximum tenant-name length accepted by the Platform convention.
///
/// The deliberately conservative bound keeps tenant identifiers small enough
/// for configuration, metrics, and operational tooling while leaving ample
/// room for descriptive deployment-owned names.
pub const MAX_FOUNDATIONDB_TENANT_NAME_LENGTH: usize = 63;

/// Validated FoundationDB tenant name owned without heap allocation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FoundationDbTenantName {
    bytes: [u8; MAX_FOUNDATIONDB_TENANT_NAME_LENGTH],
    length: u8,
}

impl FoundationDbTenantName {
    /// Validates and copies a tenant name.
    ///
    /// Names use lowercase ASCII letters, digits, and interior hyphens. This
    /// creates stable operational identifiers and prevents arbitrary external
    /// data from becoming metric labels or FoundationDB namespace names.
    pub fn new(value: &str) -> Result<Self, FoundationDbTenantNameError> {
        let value_bytes = value.as_bytes();
        if value_bytes.is_empty() {
            return Err(FoundationDbTenantNameError::Invalid {
                reason: FoundationDbTenantNameErrorReason::Empty,
            });
        }
        if value_bytes.len() > MAX_FOUNDATIONDB_TENANT_NAME_LENGTH {
            return Err(FoundationDbTenantNameError::Invalid {
                reason: FoundationDbTenantNameErrorReason::TooLong,
            });
        }
        if value_bytes.first() == Some(&b'-') || value_bytes.last() == Some(&b'-') {
            return Err(FoundationDbTenantNameError::Invalid {
                reason: FoundationDbTenantNameErrorReason::InvalidBoundary,
            });
        }
        if !value_bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        {
            return Err(FoundationDbTenantNameError::Invalid {
                reason: FoundationDbTenantNameErrorReason::InvalidCharacter,
            });
        }

        let length =
            u8::try_from(value_bytes.len()).map_err(|_| FoundationDbTenantNameError::Invalid {
                reason: FoundationDbTenantNameErrorReason::TooLong,
            })?;
        let mut bytes = [0_u8; MAX_FOUNDATIONDB_TENANT_NAME_LENGTH];
        bytes[..value_bytes.len()].copy_from_slice(value_bytes);
        Ok(Self { bytes, length })
    }

    /// Returns the validated tenant-name bytes expected by FoundationDB.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.length)]
    }
}

impl fmt::Debug for FoundationDbTenantName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FoundationDbTenantName(")?;
        fmt::Display::fmt(self, formatter)?;
        formatter.write_char(')')
    }
}

impl fmt::Display for FoundationDbTenantName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.as_bytes() {
            formatter.write_char(char::from(*byte))?;
        }
        Ok(())
    }
}

/// FoundationDB tenant-name validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FoundationDbTenantNameError {
    /// The candidate violates the stable tenant-name convention.
    #[error("invalid foundationdb tenant name: {reason}")]
    Invalid {
        /// Stable, non-sensitive validation reason.
        reason: FoundationDbTenantNameErrorReason,
    },
}

/// Stable FoundationDB tenant-name validation reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FoundationDbTenantNameErrorReason {
    /// The candidate is empty.
    #[error("empty")]
    Empty,
    /// The candidate exceeds the supported bound.
    #[error("too long")]
    TooLong,
    /// The candidate contains a character outside the allowlist.
    #[error("invalid character")]
    InvalidCharacter,
    /// The candidate begins or ends with a hyphen.
    #[error("invalid boundary")]
    InvalidBoundary,
}

#[cfg(test)]
mod tests {
    use super::{
        FoundationDbTenantName, FoundationDbTenantNameError, FoundationDbTenantNameErrorReason,
        MAX_FOUNDATIONDB_TENANT_NAME_LENGTH,
    };

    #[test]
    fn accepts_stable_operational_name() {
        let result = FoundationDbTenantName::new("search-crawl-v1");
        assert!(result.is_ok());
        let Ok(name) = result else {
            return;
        };
        assert_eq!(name.as_bytes(), b"search-crawl-v1");
        assert_eq!(name.to_string(), "search-crawl-v1");
    }

    #[test]
    fn rejects_invalid_candidates() {
        let cases = [
            ("", FoundationDbTenantNameErrorReason::Empty),
            (
                "-tenant",
                FoundationDbTenantNameErrorReason::InvalidBoundary,
            ),
            (
                "tenant-",
                FoundationDbTenantNameErrorReason::InvalidBoundary,
            ),
            (
                "Tenant",
                FoundationDbTenantNameErrorReason::InvalidCharacter,
            ),
            (
                "tenant_name",
                FoundationDbTenantNameErrorReason::InvalidCharacter,
            ),
        ];

        for (candidate, expected_reason) in cases {
            assert_eq!(
                FoundationDbTenantName::new(candidate),
                Err(FoundationDbTenantNameError::Invalid {
                    reason: expected_reason,
                })
            );
        }
    }

    #[test]
    fn rejects_oversized_candidate() {
        let candidate = "a".repeat(MAX_FOUNDATIONDB_TENANT_NAME_LENGTH + 1);
        assert_eq!(
            FoundationDbTenantName::new(candidate.as_str()),
            Err(FoundationDbTenantNameError::Invalid {
                reason: FoundationDbTenantNameErrorReason::TooLong,
            })
        );
    }
}
