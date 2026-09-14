// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Trusted proxy metadata and network-range policy.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use super::invalid_host_authority;
use crate::config::{ConfigError, ConfigValidationErrorReason};

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
