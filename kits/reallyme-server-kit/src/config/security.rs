// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::HeaderValue;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::error::{ConfigError, ConfigValidationErrorReason, HttpServerConfigField};
use super::network::NetworkPort;

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

/// Trusted external-origin normalization posture for proxy-fronted deployments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalOriginPolicyConfig {
    trusted_forwarded_host: bool,
    trusted_forwarded_proto: bool,
    require_https_external_scheme: bool,
    strict_forwarded_header_consistency: bool,
    strip_raw_proxy_headers: bool,
}

impl ExternalOriginPolicyConfig {
    /// Returns the fail-closed metadata posture.
    pub const fn secure_defaults() -> Self {
        Self {
            trusted_forwarded_host: false,
            trusted_forwarded_proto: false,
            require_https_external_scheme: false,
            strict_forwarded_header_consistency: false,
            strip_raw_proxy_headers: true,
        }
    }

    /// Constructs an explicit trusted-proxy metadata posture.
    pub const fn new(
        trusted_forwarded_host: bool,
        trusted_forwarded_proto: bool,
        require_https_external_scheme: bool,
        strict_forwarded_header_consistency: bool,
        strip_raw_proxy_headers: bool,
    ) -> Self {
        Self {
            trusted_forwarded_host,
            trusted_forwarded_proto,
            require_https_external_scheme,
            strict_forwarded_header_consistency,
            strip_raw_proxy_headers,
        }
    }

    /// Returns whether trusted peers may override the external host authority.
    pub const fn trusted_forwarded_host(self) -> bool {
        self.trusted_forwarded_host
    }

    /// Returns whether trusted peers may override the external scheme/protocol.
    pub const fn trusted_forwarded_proto(self) -> bool {
        self.trusted_forwarded_proto
    }

    /// Returns whether requests must prove an external HTTPS scheme.
    pub const fn require_https_external_scheme(self) -> bool {
        self.require_https_external_scheme
    }

    /// Returns whether conflicting trusted forwarded header families fail closed.
    pub const fn strict_forwarded_header_consistency(self) -> bool {
        self.strict_forwarded_header_consistency
    }

    /// Returns whether raw proxy headers are stripped before app handlers run.
    pub const fn strip_raw_proxy_headers(self) -> bool {
        self.strip_raw_proxy_headers
    }
}

/// Backward-compatible alias for trusted proxy metadata normalization posture.
pub type TrustedProxyRequestMetadataConfig = ExternalOriginPolicyConfig;

/// Trusted proxy header handling policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustedProxyHeaders {
    /// Strip forwarded/proxy identity headers before app handlers observe the request.
    IgnoreAll,
    /// Trust forwarded/proxy identity headers only for configured peer ranges.
    ///
    /// This should only be used when the server process is reachable solely
    /// through a trusted ingress that overwrites these headers.
    TrustConfiguredProxies(Vec<TrustedProxyRange>),
}

impl TrustedProxyHeaders {
    /// Returns the fail-closed default.
    pub const fn ignore_all() -> Self {
        Self::IgnoreAll
    }

    /// Constructs a trusted-proxy policy from explicit peer ranges.
    pub fn trust_configured_proxies(ranges: Vec<TrustedProxyRange>) -> Result<Self, ConfigError> {
        if ranges.is_empty() {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::MustBeNonEmpty,
            ));
        }

        Ok(Self::TrustConfiguredProxies(ranges))
    }

    /// Returns whether forwarded/proxy identity headers may be trusted.
    pub fn trusts_peer(&self, peer_ip: Option<IpAddr>) -> bool {
        match (self, peer_ip) {
            (Self::IgnoreAll, _) | (Self::TrustConfiguredProxies(_), None) => false,
            (Self::TrustConfiguredProxies(ranges), Some(peer_ip)) => {
                ranges.iter().any(|range| range.contains(peer_ip))
            }
        }
    }
}

/// Validated trusted proxy IP address or CIDR range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedProxyRange {
    network: IpAddr,
    prefix_len: u8,
}

impl TrustedProxyRange {
    /// Parses an exact IP address or CIDR range.
    pub fn parse(value: &str) -> Result<Self, ConfigError> {
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::InvalidNetworkRange,
            ));
        }

        match value.split_once('/') {
            Some((address, prefix)) => {
                let network = address.parse::<IpAddr>().map_err(|_| {
                    invalid_host_authority(ConfigValidationErrorReason::InvalidNetworkRange)
                })?;
                let prefix_len = prefix.parse::<u8>().map_err(|_| {
                    invalid_host_authority(ConfigValidationErrorReason::InvalidNetworkRange)
                })?;

                Self::new(network, prefix_len)
            }
            None => {
                let network = value.parse::<IpAddr>().map_err(|_| {
                    invalid_host_authority(ConfigValidationErrorReason::InvalidNetworkRange)
                })?;
                let prefix_len = match network {
                    IpAddr::V4(_) => 32,
                    IpAddr::V6(_) => 128,
                };

                Ok(Self {
                    network,
                    prefix_len,
                })
            }
        }
    }

    fn new(network: IpAddr, prefix_len: u8) -> Result<Self, ConfigError> {
        let max_prefix_len = match network {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };

        if prefix_len > max_prefix_len {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::InvalidNetworkRange,
            ));
        }

        Ok(Self {
            network,
            prefix_len,
        })
    }

    /// Returns whether the supplied peer IP is within this trusted range.
    pub fn contains(self, peer_ip: IpAddr) -> bool {
        match (self.network, peer_ip) {
            (IpAddr::V4(network), IpAddr::V4(peer)) => {
                ip_v4_prefix_matches(network, peer, self.prefix_len)
            }
            (IpAddr::V6(network), IpAddr::V6(peer)) => {
                ip_v6_prefix_matches(network, peer, self.prefix_len)
            }
            _ => false,
        }
    }
}

fn ip_v4_prefix_matches(network: Ipv4Addr, peer: Ipv4Addr, prefix_len: u8) -> bool {
    let mask = prefix_mask_u32(prefix_len);

    u32::from(network) & mask == u32::from(peer) & mask
}

fn ip_v6_prefix_matches(network: Ipv6Addr, peer: Ipv6Addr, prefix_len: u8) -> bool {
    let mask = prefix_mask_u128(prefix_len);

    u128::from(network) & mask == u128::from(peer) & mask
}

fn prefix_mask_u32(prefix_len: u8) -> u32 {
    if prefix_len == 0 {
        0
    } else {
        u32::MAX << (32 - u32::from(prefix_len))
    }
}

fn prefix_mask_u128(prefix_len: u8) -> u128 {
    if prefix_len == 0 {
        0
    } else {
        u128::MAX << (128 - u32::from(prefix_len))
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

fn validate_host_authority_syntax(value: &str) -> Result<(), ConfigError> {
    if value.is_empty() {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::MustBeNonEmpty,
        ));
    }

    if value.len() > MAX_HOST_AUTHORITY_BYTES {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        ));
    }

    if value.chars().any(char::is_whitespace)
        || value.contains('/')
        || value.contains('?')
        || value.contains('#')
        || value.contains('@')
        || value.contains("://")
    {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::InvalidOrigin,
        ));
    }

    HeaderValue::from_str(value)
        .map_err(|_| invalid_host_authority(ConfigValidationErrorReason::InvalidHeaderValue))?;

    Ok(())
}

fn parse_host_authority_parts(authority: &str) -> Result<HostAuthorityParts<'_>, ConfigError> {
    if let Some(remainder) = authority.strip_prefix('[') {
        let Some((host, suffix)) = remainder.split_once(']') else {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::InvalidHeaderValue,
            ));
        };
        if host.is_empty() {
            return Err(invalid_host_authority(
                ConfigValidationErrorReason::InvalidHeaderValue,
            ));
        }

        let port = if suffix.is_empty() {
            None
        } else {
            let Some(port_value) = suffix.strip_prefix(':') else {
                return Err(invalid_host_authority(
                    ConfigValidationErrorReason::InvalidHeaderValue,
                ));
            };
            Some(parse_host_authority_port(port_value)?)
        };

        return Ok(HostAuthorityParts { host, port });
    }

    if let Some((host, port_value)) = authority.rsplit_once(':')
        && !host.contains(':')
        && !port_value.is_empty()
        && port_value.chars().all(|char| char.is_ascii_digit())
    {
        return Ok(HostAuthorityParts {
            host,
            port: Some(parse_host_authority_port(port_value)?),
        });
    }

    if authority.contains(':') {
        return Err(invalid_host_authority(
            ConfigValidationErrorReason::InvalidHeaderValue,
        ));
    }

    Ok(HostAuthorityParts {
        host: authority,
        port: None,
    })
}

fn parse_host_authority_port(value: &str) -> Result<NetworkPort, ConfigError> {
    let port = value
        .parse::<u16>()
        .map_err(|_| invalid_host_authority(ConfigValidationErrorReason::InvalidHeaderValue))?;

    NetworkPort::new(port)
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::{
        DEFAULT_HTTP_HEADER_BYTES_LIMIT_VALUE, DEFAULT_HTTP_HEADER_COUNT_LIMIT_VALUE,
        HostAuthority, HostAuthorityPolicy, HttpHeaderBytesLimit, HttpHeaderCountLimit,
        HttpSecurityConfig, MAX_HTTP_HEADER_BYTES_LIMIT_VALUE, MAX_HTTP_HEADER_COUNT_LIMIT_VALUE,
        TrustedProxyHeaders, TrustedProxyRange, TrustedProxyRequestMetadataConfig,
    };
    use crate::config::{
        ConfigError, ConfigValidationErrorReason, HttpServerConfigField, NetworkPort,
        OperationalRouteAccess,
    };

    #[test]
    fn security_defaults_fail_closed_for_operational_routes_and_proxy_headers() {
        let config = HttpSecurityConfig::secure_defaults();

        assert!(config.security_headers().enabled());
        assert_eq!(
            config.header_limits().max_header_count().as_usize(),
            DEFAULT_HTTP_HEADER_COUNT_LIMIT_VALUE
        );
        assert_eq!(
            config.header_limits().max_header_bytes().as_usize(),
            DEFAULT_HTTP_HEADER_BYTES_LIMIT_VALUE
        );
        assert_eq!(
            config.trusted_proxy_headers(),
            &TrustedProxyHeaders::IgnoreAll
        );
        assert_eq!(
            config.trusted_proxy_request_metadata(),
            TrustedProxyRequestMetadataConfig::secure_defaults()
        );
        assert_eq!(
            config.operational_route_access(),
            OperationalRouteAccess::LocalOnly
        );
    }

    #[test]
    fn host_authority_exposes_host_and_port_components() {
        let authority = HostAuthority::new("API.ReallyMe.Net:443").expect("valid authority");

        assert_eq!(authority.as_str(), "api.reallyme.net:443");
        assert_eq!(authority.host(), "api.reallyme.net");
        assert_eq!(authority.port().map(NetworkPort::as_u16), Some(443));
        assert!(authority.has_explicit_port());
    }

    #[test]
    fn host_authority_accepts_host_only_authorities() {
        let authority = HostAuthority::new("API.ReallyMe.Net").expect("valid authority");

        assert_eq!(authority.as_str(), "api.reallyme.net");
        assert_eq!(authority.host(), "api.reallyme.net");
        assert_eq!(authority.port(), None);
        assert!(!authority.has_explicit_port());
    }

    #[test]
    fn host_authority_validation_rejects_non_authority_shapes() {
        for value in [
            "",
            "api.reallyme.net/path",
            "https://api.reallyme.net",
            "api.reallyme.net?x=1",
            "api.reallyme.net#fragment",
            "user@api.reallyme.net",
            "api reallyme net",
        ] {
            assert!(
                HostAuthority::new(value).is_err(),
                "authority should be rejected: {value}"
            );
        }
    }

    #[test]
    fn host_authority_policy_matches_case_insensitively() {
        let policy = HostAuthorityPolicy::allow_list(vec![
            HostAuthority::new("API.ReallyMe.Net:443").expect("valid authority"),
        ])
        .expect("non-empty allowlist");

        assert!(policy.allows("api.reallyme.net:443"));
        assert!(!policy.allows("admin.reallyme.net:443"));
    }

    #[test]
    fn allow_any_still_rejects_malformed_authority_values() {
        let policy = HostAuthorityPolicy::allow_any();

        assert!(policy.allows("api.reallyme.net"));
        assert!(!policy.allows("https://api.reallyme.net"));
        assert!(!policy.allows("api.reallyme.net/path"));
        assert!(!policy.allows("api reallyme net"));
    }

    #[test]
    fn host_authority_allowlist_rejects_empty_list() {
        assert_eq!(
            HostAuthorityPolicy::allow_list(Vec::new()),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::MustBeNonEmpty,
            })
        );
    }

    #[test]
    fn header_limits_accept_valid_boundary_values() {
        assert_eq!(
            HttpHeaderCountLimit::new(MAX_HTTP_HEADER_COUNT_LIMIT_VALUE)
                .expect("max count boundary should be valid")
                .as_usize(),
            MAX_HTTP_HEADER_COUNT_LIMIT_VALUE
        );
        assert_eq!(
            HttpHeaderBytesLimit::new(MAX_HTTP_HEADER_BYTES_LIMIT_VALUE)
                .expect("max bytes boundary should be valid")
                .as_usize(),
            MAX_HTTP_HEADER_BYTES_LIMIT_VALUE
        );
    }

    #[test]
    fn header_limits_reject_zero_too_small_and_too_large_values() {
        assert_eq!(
            HttpHeaderCountLimit::new(0),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
            })
        );
        assert_eq!(
            HttpHeaderCountLimit::new(MAX_HTTP_HEADER_COUNT_LIMIT_VALUE + 1),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            })
        );
        assert_eq!(
            HttpHeaderBytesLimit::new(1),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::MustBeAtLeastMinimum,
            })
        );
        assert_eq!(
            HttpHeaderBytesLimit::new(MAX_HTTP_HEADER_BYTES_LIMIT_VALUE + 1),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            })
        );
    }

    #[test]
    fn trusted_proxy_ranges_match_ipv4_and_ipv6_prefixes() {
        let ipv4 = TrustedProxyRange::parse("10.0.0.0/8").expect("valid ipv4 range");
        let ipv6 = TrustedProxyRange::parse("2001:db8::/32").expect("valid ipv6 range");

        assert!(ipv4.contains(IpAddr::V4(Ipv4Addr::new(10, 42, 0, 1))));
        assert!(!ipv4.contains(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))));
        assert!(ipv6.contains(IpAddr::V6(
            "2001:db8::1".parse::<Ipv6Addr>().expect("valid ipv6")
        )));
        assert!(!ipv6.contains(IpAddr::V6(
            "2001:db9::1".parse::<Ipv6Addr>().expect("valid ipv6")
        )));
    }

    #[test]
    fn trusted_proxy_headers_require_configured_ranges() {
        assert_eq!(
            TrustedProxyHeaders::trust_configured_proxies(Vec::new()),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::MustBeNonEmpty,
            })
        );
    }

    #[test]
    fn invalid_trusted_proxy_ranges_are_typed_config_errors() {
        assert_eq!(
            TrustedProxyRange::parse("10.0.0.0/99"),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::InvalidNetworkRange,
            })
        );
        assert_eq!(
            TrustedProxyRange::parse("not-an-ip-range"),
            Err(ConfigError::InvalidHttpServerConfig {
                field: HttpServerConfigField::Security,
                reason: ConfigValidationErrorReason::InvalidNetworkRange,
            })
        );
    }
}
