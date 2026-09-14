// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use super::error::{ConfigError, ConfigValidationErrorReason, HttpServerConfigField};
use super::network::NetworkPort;

mod host_authority_validation;
mod proxy;

use host_authority_validation::{parse_host_authority_parts, validate_host_authority_syntax};
pub use proxy::{
    ExternalOriginPolicyConfig, TrustedProxyHeaders, TrustedProxyRange,
    TrustedProxyRequestMetadataConfig,
};

const MAX_HOST_AUTHORITY_BYTES: usize = 255;
/// Default maximum number of HTTP headers accepted before app handlers run.
pub const DEFAULT_HTTP_HEADER_COUNT_LIMIT_VALUE: usize = 100;
/// Default aggregate HTTP header name/value byte budget.
pub const DEFAULT_HTTP_HEADER_BYTES_LIMIT_VALUE: usize = 64 * 1024;
/// Platform maximum HTTP header count.
pub const MAX_HTTP_HEADER_COUNT_LIMIT_VALUE: usize = 256;
/// Platform maximum aggregate HTTP header name/value byte budget.
pub const MAX_HTTP_HEADER_BYTES_LIMIT_VALUE: usize = 256 * 1024;
const MIN_HTTP_HEADER_BYTES_LIMIT_VALUE: usize = 1024;

/// Validated HTTP security posture for a server process.
///
/// This config is intentionally generic and transport-focused. Product
/// authentication, authorization, tenancy, and business abuse policy belong in
/// app crates. Server-kit owns this layer because it protects every route
/// before app behavior runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSecurityConfig {
    security_headers: SecurityHeadersConfig,
    host_authority_policy: HostAuthorityPolicy,
    trusted_proxy_headers: TrustedProxyHeaders,
    external_origin_policy: ExternalOriginPolicyConfig,
    operational_route_access: OperationalRouteAccess,
    header_limits: HttpHeaderLimitConfig,
}

impl HttpSecurityConfig {
    /// Constructs the safe default HTTP security posture.
    pub fn new(
        security_headers: SecurityHeadersConfig,
        host_authority_policy: HostAuthorityPolicy,
        trusted_proxy_headers: TrustedProxyHeaders,
        external_origin_policy: ExternalOriginPolicyConfig,
        operational_route_access: OperationalRouteAccess,
    ) -> Self {
        Self {
            security_headers,
            host_authority_policy,
            trusted_proxy_headers,
            external_origin_policy,
            operational_route_access,
            header_limits: HttpHeaderLimitConfig::secure_defaults(),
        }
    }

    /// Returns the default fail-closed HTTP security posture.
    pub fn secure_defaults() -> Self {
        Self {
            security_headers: SecurityHeadersConfig::secure_defaults(),
            host_authority_policy: HostAuthorityPolicy::allow_any(),
            trusted_proxy_headers: TrustedProxyHeaders::ignore_all(),
            external_origin_policy: ExternalOriginPolicyConfig::secure_defaults(),
            operational_route_access: OperationalRouteAccess::LocalOnly,
            header_limits: HttpHeaderLimitConfig::secure_defaults(),
        }
    }

    /// Returns a copy of this config with explicit HTTP header limits.
    pub fn with_header_limits(mut self, header_limits: HttpHeaderLimitConfig) -> Self {
        self.header_limits = header_limits;
        self
    }

    /// Returns security response-header config.
    pub const fn security_headers(&self) -> SecurityHeadersConfig {
        self.security_headers
    }

    /// Returns host/authority validation policy.
    pub fn host_authority_policy(&self) -> &HostAuthorityPolicy {
        &self.host_authority_policy
    }

    /// Returns whether forwarded/proxy identity headers should be trusted.
    pub fn trusted_proxy_headers(&self) -> &TrustedProxyHeaders {
        &self.trusted_proxy_headers
    }

    /// Returns trusted external-origin normalization posture.
    pub const fn external_origin_policy(&self) -> ExternalOriginPolicyConfig {
        self.external_origin_policy
    }

    /// Returns trusted external-request metadata normalization posture.
    pub const fn trusted_proxy_request_metadata(&self) -> TrustedProxyRequestMetadataConfig {
        self.external_origin_policy
    }

    /// Returns operational route exposure policy.
    pub const fn operational_route_access(&self) -> OperationalRouteAccess {
        self.operational_route_access
    }

    /// Returns HTTP header admission limits.
    pub const fn header_limits(&self) -> HttpHeaderLimitConfig {
        self.header_limits
    }
}

impl Default for HttpSecurityConfig {
    fn default() -> Self {
        Self::secure_defaults()
    }
}

/// HTTP security response-header policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecurityHeadersConfig {
    enabled: bool,
}

impl SecurityHeadersConfig {
    /// Enables standard API-safe security response headers.
    pub const fn secure_defaults() -> Self {
        Self { enabled: true }
    }

    /// Disables automatic security response headers.
    ///
    /// This is intended only for tests or unusual host integrations where an
    /// upstream security gateway injects equivalent headers. Normal server
    /// processes should keep the secure default enabled.
    pub const fn disabled() -> Self {
        Self { enabled: false }
    }

    /// Returns whether standard security response headers are enabled.
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

/// Validated HTTP header admission limits.
///
/// The native server runtime applies these before app handlers run. This is a
/// transport-level slowloris/header-bloat guard, not a product abuse policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpHeaderLimitConfig {
    max_header_count: HttpHeaderCountLimit,
    max_header_bytes: HttpHeaderBytesLimit,
}

impl HttpHeaderLimitConfig {
    /// Constructs validated HTTP header admission limits.
    pub const fn new(
        max_header_count: HttpHeaderCountLimit,
        max_header_bytes: HttpHeaderBytesLimit,
    ) -> Self {
        Self {
            max_header_count,
            max_header_bytes,
        }
    }

    /// Returns conservative default HTTP header limits.
    pub const fn secure_defaults() -> Self {
        Self {
            max_header_count: DEFAULT_HTTP_HEADER_COUNT_LIMIT,
            max_header_bytes: DEFAULT_HTTP_HEADER_BYTES_LIMIT,
        }
    }

    /// Returns the maximum accepted header count.
    pub const fn max_header_count(self) -> HttpHeaderCountLimit {
        self.max_header_count
    }

    /// Returns the aggregate accepted header name/value byte budget.
    pub const fn max_header_bytes(self) -> HttpHeaderBytesLimit {
        self.max_header_bytes
    }
}

/// Validated maximum HTTP header count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpHeaderCountLimit(usize);

impl HttpHeaderCountLimit {
    /// Constructs a validated HTTP header-count limit.
    pub fn new(value: usize) -> Result<Self, ConfigError> {
        if value == 0 {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeGreaterThanZero,
            ));
        }

        if value > MAX_HTTP_HEADER_COUNT_LIMIT_VALUE {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            ));
        }

        Ok(Self(value))
    }

    /// Returns the validated limit.
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

/// Validated aggregate HTTP header byte limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpHeaderBytesLimit(usize);

impl HttpHeaderBytesLimit {
    /// Constructs a validated aggregate HTTP header byte limit.
    pub fn new(value: usize) -> Result<Self, ConfigError> {
        if value == 0 {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeGreaterThanZero,
            ));
        }

        if value < MIN_HTTP_HEADER_BYTES_LIMIT_VALUE {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeAtLeastMinimum,
            ));
        }

        if value > MAX_HTTP_HEADER_BYTES_LIMIT_VALUE {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            ));
        }

        Ok(Self(value))
    }

    /// Returns the validated limit.
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

/// Default validated HTTP header-count limit.
pub const DEFAULT_HTTP_HEADER_COUNT_LIMIT: HttpHeaderCountLimit =
    HttpHeaderCountLimit(DEFAULT_HTTP_HEADER_COUNT_LIMIT_VALUE);
/// Default validated aggregate HTTP header byte limit.
pub const DEFAULT_HTTP_HEADER_BYTES_LIMIT: HttpHeaderBytesLimit =
    HttpHeaderBytesLimit(DEFAULT_HTTP_HEADER_BYTES_LIMIT_VALUE);

/// Validated host or HTTP/2 `:authority` value.
#[derive(Clone, PartialEq, Eq)]
pub struct HostAuthority {
    normalized: String,
    host: String,
    port: Option<NetworkPort>,
}

/// Borrowed, validated host-authority components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct HostAuthorityParts<'a> {
    host: &'a str,
    port: Option<NetworkPort>,
}

impl<'a> HostAuthorityParts<'a> {
    /// Returns the borrowed host name or IP literal without an authority port.
    pub(crate) const fn host(self) -> &'a str {
        self.host
    }

    /// Returns the explicit authority port, if one was present.
    pub(crate) const fn port(self) -> Option<NetworkPort> {
        self.port
    }
}

impl HostAuthority {
    /// Constructs a validated host authority.
    pub fn new(value: impl Into<String>) -> Result<Self, ConfigError> {
        let value = value.into();

        Self::validate_authority(value.as_str())?;

        let normalized = value.to_ascii_lowercase();
        let parts = parse_host_authority_parts(normalized.as_str())?;
        let host = parts.host().to_owned();
        let port = parts.port();

        Ok(Self {
            normalized,
            host,
            port,
        })
    }

    /// Validates and retains a borrowed authority value.
    ///
    /// Forwarded-header paths often receive a borrowed `&str` from header
    /// parsing. This constructor avoids first copying that borrowed value into
    /// a temporary `String`; it allocates only the normalized representation
    /// that the retained [`HostAuthority`] must own.
    pub(crate) fn retain_authority(value: &str) -> Result<Self, ConfigError> {
        Self::validate_authority(value)?;

        let normalized = value.to_ascii_lowercase();
        let parts = parse_host_authority_parts(normalized.as_str())?;
        let host = parts.host().to_owned();
        let port = parts.port();

        Ok(Self {
            normalized,
            host,
            port,
        })
    }

    /// Validates a borrowed authority without retaining or normalizing it.
    pub fn validate_authority(value: &str) -> Result<(), ConfigError> {
        validate_host_authority_syntax(value)?;
        let _parts = parse_host_authority_parts(value)?;
        Ok(())
    }

    /// Parses borrowed authority components after applying validation.
    pub(crate) fn authority_parts(value: &str) -> Result<HostAuthorityParts<'_>, ConfigError> {
        validate_host_authority_syntax(value)?;
        parse_host_authority_parts(value)
    }

    /// Returns the normalized authority.
    pub fn as_str(&self) -> &str {
        self.normalized.as_str()
    }

    /// Returns the normalized host name or IP literal without an authority port.
    pub fn host(&self) -> &str {
        self.host.as_str()
    }

    /// Returns the explicit authority port, if one was present.
    pub const fn port(&self) -> Option<NetworkPort> {
        self.port
    }

    /// Returns whether the authority carried an explicit port.
    pub const fn has_explicit_port(&self) -> bool {
        self.port.is_some()
    }
}

impl fmt::Debug for HostAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostAuthority")
            .field("value", &self.normalized)
            .finish()
    }
}

/// Host/authority allowlist policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostAuthorityPolicy {
    /// Any syntactically valid host is allowed.
    ///
    /// This is the compatibility default because some deployments sit behind
    /// gateways that rewrite host/authority. Public deployments should prefer
    /// [`Self::allow_list`] once their ingress hostnames are known.
    Any,
    /// Only the configured authorities are allowed.
    AllowList(Vec<HostAuthority>),
}

impl HostAuthorityPolicy {
    /// Allows any syntactically valid host.
    pub const fn allow_any() -> Self {
        Self::Any
    }

    /// Constructs an exact host/authority allowlist.
    pub fn allow_list(authorities: Vec<HostAuthority>) -> Result<Self, ConfigError> {
        if authorities.is_empty() {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeNonEmpty,
            ));
        }

        Ok(Self::AllowList(authorities))
    }

    /// Returns whether the supplied host/authority is allowed.
    pub fn allows(&self, authority: &str) -> bool {
        let Ok(normalized) = HostAuthority::new(authority.to_owned()) else {
            return false;
        };

        self.allows_normalized(&normalized)
    }

    /// Returns whether the supplied already-validated authority is allowed.
    pub fn allows_normalized(&self, authority: &HostAuthority) -> bool {
        match self {
            Self::Any => true,
            Self::AllowList(allowed) => allowed
                .iter()
                .any(|candidate| candidate.as_str() == authority.as_str()),
        }
    }
}

/// Exposure policy for runtime-owned operational HTTP routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalRouteAccess {
    /// Operational routes are reachable only from loopback clients.
    LocalOnly,
    /// Operational routes are reachable from any client.
    ///
    /// This is useful behind trusted gateways that enforce their own access
    /// control, but it should be an explicit deployment choice.
    Public,
}

impl OperationalRouteAccess {
    /// Returns the fail-closed default.
    pub const fn local_only() -> Self {
        Self::LocalOnly
    }
}

fn invalid_host_authority(reason: ConfigValidationErrorReason) -> ConfigError {
    ConfigError::InvalidHttpServerConfig {
        field: HttpServerConfigField::Security,
        reason,
    }
}

#[cfg(test)]
#[path = "security/tests.rs"]
mod tests;
