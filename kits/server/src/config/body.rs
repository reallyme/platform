// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::num::NonZeroUsize;

use super::error::{BodyLimitConfigField, ConfigError, ConfigValidationErrorReason};

/// Validated request body limit.
///
/// Requires a limit greater than zero. Hosts must choose an appropriate upper
/// bound for each service or endpoint; this type imposes no shared maximum
/// because acceptable body sizes depend on the workload.
///
/// # Examples
///
/// ```rust
/// use reallyme_server_kit::config::{BodyLimitConfig, RequestBodyLimitBytes};
///
/// let body_limit = RequestBodyLimitBytes::new(1024 * 1024)?;
/// let config = BodyLimitConfig::new(body_limit);
///
/// assert_eq!(config.request_body_limit().as_usize(), 1024 * 1024);
/// # Ok::<(), reallyme_server_kit::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestBodyLimitBytes(NonZeroUsize);

impl RequestBodyLimitBytes {
    /// Constructs a validated request body limit.
    pub fn new(value: usize) -> Result<Self, ConfigError> {
        match NonZeroUsize::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(ConfigError::InvalidBodyLimitConfig {
                field: BodyLimitConfigField::RequestBodyLimitBytes,
                reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
            }),
        }
    }

    /// Returns the body limit in bytes.
    pub fn as_usize(self) -> usize {
        self.0.get()
    }
}

/// Shared request body limit configuration.
///
/// This type intentionally does not implement `Default`. Apps should
/// either choose an explicit limit themselves or opt into the documented
/// server-kit baseline via `DEFAULT_REQUEST_BODY_LIMIT_BYTES` during their own
/// config loading path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyLimitConfig {
    request_body_limit: RequestBodyLimitBytes,
}

impl BodyLimitConfig {
    /// Constructs validated body-limit configuration.
    pub fn new(request_body_limit: RequestBodyLimitBytes) -> Self {
        Self { request_body_limit }
    }

    /// Returns the validated request body limit.
    pub fn request_body_limit(&self) -> RequestBodyLimitBytes {
        self.request_body_limit
    }
}
