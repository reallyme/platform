// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use crate::config::SecretString;

/// Generic bearer-token credentials.
pub struct BearerToken(SecretString);

impl BearerToken {
    /// Creates a bearer-token wrapper.
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::new(value.into()))
    }

    /// Returns the wrapped secret token by explicit reference.
    ///
    /// Callers must not log, format, or include this value in errors,
    /// telemetry, metrics, or transport responses.
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret().as_str()
    }

    /// Compares a candidate token using constant-time equality.
    ///
    /// Callers must not rely on `==` with this secret.
    pub fn constant_time_eq(&self, candidate: &str) -> bool {
        self.0.constant_time_eq(candidate)
    }
}

impl fmt::Debug for BearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BearerToken([REDACTED])")
    }
}

/// Generic API-key credentials.
pub struct ApiKey(SecretString);

impl ApiKey {
    /// Creates an API-key wrapper.
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::new(value.into()))
    }

    /// Returns the wrapped secret key by explicit reference.
    ///
    /// Callers must not log, format, or include this value in errors,
    /// telemetry, metrics, or transport responses.
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret().as_str()
    }

    /// Compares a candidate API key using constant-time equality.
    ///
    /// Callers must not rely on `==` with this secret.
    pub fn constant_time_eq(&self, candidate: &str) -> bool {
        self.0.constant_time_eq(candidate)
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey([REDACTED])")
    }
}

/// Generic service-token credentials.
pub struct ServiceToken(SecretString);

impl ServiceToken {
    /// Creates a service-token wrapper.
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::new(value.into()))
    }

    /// Returns the wrapped secret token by explicit reference.
    ///
    /// Callers must not log, format, or include this value in errors,
    /// telemetry, metrics, or transport responses.
    pub fn expose_secret(&self) -> &str {
        self.0.expose_secret().as_str()
    }

    /// Compares a candidate token using constant-time equality.
    ///
    /// Callers must not rely on `==` with this secret.
    pub fn constant_time_eq(&self, candidate: &str) -> bool {
        self.0.constant_time_eq(candidate)
    }
}

impl fmt::Debug for ServiceToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServiceToken([REDACTED])")
    }
}

/// Generic authentication credentials accepted by infrastructure adapters.
///
/// This enum intentionally models only credential *shapes*. It does not
/// implement JWT validation, user-database lookup, or product-specific token
/// semantics.
pub enum Credentials {
    /// Explicit anonymous request.
    Anonymous,
    /// Generic bearer token.
    BearerToken(BearerToken),
    /// Generic API key.
    ApiKey(ApiKey),
    /// Generic service token.
    ServiceToken(ServiceToken),
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anonymous => formatter.write_str("Credentials::Anonymous"),
            Self::BearerToken(token) => formatter
                .debug_tuple("Credentials::BearerToken")
                .field(token)
                .finish(),
            Self::ApiKey(key) => formatter
                .debug_tuple("Credentials::ApiKey")
                .field(key)
                .finish(),
            Self::ServiceToken(token) => formatter
                .debug_tuple("Credentials::ServiceToken")
                .field(token)
                .finish(),
        }
    }
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
