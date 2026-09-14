// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::auth::validate_symbolic_name;

use super::{AppKitError, AppKitField};

const MAX_APP_ERROR_CODE_BYTES: usize = 128;

/// Validated stable app error code.
///
/// Concrete app crates define their own error catalogs using this primitive.
/// Codes must be low-cardinality and safe to expose through transport-specific
/// public error mappings.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppErrorCode(String);

impl AppErrorCode {
    /// Constructs a validated app error code.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_symbolic_name(
            value.as_str(),
            AppKitField::ErrorCode,
            MAX_APP_ERROR_CODE_BYTES,
        )?;

        Ok(Self(value))
    }

    /// Returns the validated error code.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppErrorCode")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Stable app error category for public mapping layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorCategory {
    /// Caller supplied invalid input.
    InvalidRequest,
    /// Caller is not authenticated.
    Unauthenticated,
    /// Caller is authenticated but not authorized.
    PermissionDenied,
    /// Requested resource was not found.
    NotFound,
    /// Request conflicts with current state.
    Conflict,
    /// Caller or system exceeded a bounded limit.
    ResourceExhausted,
    /// Downstream port or app component is unavailable.
    Unavailable,
    /// Operation exceeded a bounded deadline.
    DeadlineExceeded,
    /// Internal app failure. Public transports must not expose details.
    Internal,
}

/// Retry guidance for app errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorRetryDisposition {
    /// Retrying without a state/config/input change is not expected to help.
    NotRetryable,
    /// Retrying later may succeed.
    Retryable,
}

/// Host-neutral app error contract descriptor.
///
/// This descriptor intentionally contains no free-form messages or dynamic
/// context. Transport adapters map it to HTTP, gRPC, Connect RPC, or Worker
/// error responses without leaking internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppErrorContract {
    code: AppErrorCode,
    category: AppErrorCategory,
    retry: AppErrorRetryDisposition,
}

impl AppErrorContract {
    /// Constructs an app error contract.
    pub const fn new(
        code: AppErrorCode,
        category: AppErrorCategory,
        retry: AppErrorRetryDisposition,
    ) -> Self {
        Self {
            code,
            category,
            retry,
        }
    }

    /// Returns the stable app error code.
    pub const fn code(&self) -> &AppErrorCode {
        &self.code
    }

    /// Returns the stable app error category.
    pub const fn category(&self) -> AppErrorCategory {
        self.category
    }

    /// Returns retry guidance.
    pub const fn retry(&self) -> AppErrorRetryDisposition {
        self.retry
    }
}

#[cfg(test)]
#[path = "contract_tests.rs"]
mod tests;
