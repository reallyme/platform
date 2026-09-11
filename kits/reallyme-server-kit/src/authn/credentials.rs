// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::{ApiKey, BearerToken, Credentials, ServiceToken};

    #[test]
    fn credentials_debug_redacts_secret_values() {
        let bearer = format!(
            "{:?}",
            Credentials::BearerToken(BearerToken::new("bearer-secret"))
        );
        let api_key = format!("{:?}", Credentials::ApiKey(ApiKey::new("api-key-secret")));
        let service = format!(
            "{:?}",
            Credentials::ServiceToken(ServiceToken::new("service-token-secret"))
        );

        assert!(!bearer.contains("bearer-secret"));
        assert!(!api_key.contains("api-key-secret"));
        assert!(!service.contains("service-token-secret"));
        assert!(bearer.contains("[REDACTED]"));
        assert!(api_key.contains("[REDACTED]"));
        assert!(service.contains("[REDACTED]"));
    }

    #[test]
    fn credential_constant_time_eq_covers_secret_comparison_edges() {
        {
            let token = BearerToken::new("bearer-secret");
            assert!(token.constant_time_eq("bearer-secret"));
            assert!(!token.constant_time_eq("bearer-other"));
            assert!(!token.constant_time_eq("bearer-secret-extra"));
            assert!(BearerToken::new("").constant_time_eq(""));
        }

        {
            let key = ApiKey::new("api-key-secret");
            assert!(key.constant_time_eq("api-key-secret"));
            assert!(!key.constant_time_eq("api-key-other"));
            assert!(!key.constant_time_eq("api-key-secret-extra"));
            assert!(ApiKey::new("").constant_time_eq(""));
        }

        {
            let token = ServiceToken::new("service-token-secret");
            assert!(token.constant_time_eq("service-token-secret"));
            assert!(!token.constant_time_eq("service-token-other"));
            assert!(!token.constant_time_eq("service-token-secret-extra"));
            assert!(ServiceToken::new("").constant_time_eq(""));
        }
    }
}
