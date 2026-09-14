// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::{
    DEFAULT_HTTP_HEADER_BYTES_LIMIT_VALUE, DEFAULT_HTTP_HEADER_COUNT_LIMIT_VALUE, HostAuthority,
    HostAuthorityPolicy, HttpHeaderBytesLimit, HttpHeaderCountLimit, HttpSecurityConfig,
    MAX_HTTP_HEADER_BYTES_LIMIT_VALUE, MAX_HTTP_HEADER_COUNT_LIMIT_VALUE, TrustedProxyHeaders,
    TrustedProxyRange, TrustedProxyRequestMetadataConfig,
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
