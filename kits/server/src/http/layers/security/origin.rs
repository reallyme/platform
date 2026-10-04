// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::IpAddr;

use crate::config::{HostAuthority, NetworkPort};

/// Normalized client IP extracted from trusted proxy metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForwardedClientIp(pub(super) IpAddr);

impl ForwardedClientIp {
    /// Returns the normalized client IP.
    pub const fn into_ip_addr(self) -> IpAddr {
        self.0
    }
}

/// Normalized external host authority selected from trusted proxy metadata or the direct request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardedHost(HostAuthority);

impl ForwardedHost {
    /// Constructs normalized host metadata from a validated authority.
    pub const fn new(authority: HostAuthority) -> Self {
        Self(authority)
    }

    /// Returns the validated external authority.
    pub const fn authority(&self) -> &HostAuthority {
        &self.0
    }

    /// Returns the normalized external host name or IP literal.
    pub fn host(&self) -> &str {
        self.0.host()
    }

    /// Returns the explicit external port, if one was present.
    pub const fn port(&self) -> Option<NetworkPort> {
        self.0.port()
    }
}

/// Normalized external scheme selected from trusted proxy metadata or the direct request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForwardedProto {
    /// HTTP.
    Http,
    /// HTTPS.
    Https,
}

impl ForwardedProto {
    /// Returns the stable lowercase scheme string.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    /// Returns whether the normalized external scheme is HTTPS.
    pub const fn is_https(self) -> bool {
        matches!(self, Self::Https)
    }
}

/// Safely-derived external request origin assembled from normalized scheme and authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRequestOrigin {
    host: ForwardedHost,
    proto: ForwardedProto,
}

impl ExternalRequestOrigin {
    /// Constructs a normalized external origin.
    pub const fn new(host: ForwardedHost, proto: ForwardedProto) -> Self {
        Self { host, proto }
    }

    /// Returns the normalized external authority.
    pub const fn host(&self) -> &ForwardedHost {
        &self.host
    }

    /// Returns the normalized external scheme.
    pub const fn proto(&self) -> ForwardedProto {
        self.proto
    }

    /// Returns the explicit external port, if one was present.
    pub const fn port(&self) -> Option<NetworkPort> {
        self.host.port()
    }

    /// Returns the safely-derived base URL for the external request origin.
    pub fn base_url(&self) -> String {
        format!(
            "{}://{}",
            self.proto.as_str(),
            self.host.authority().as_str()
        )
    }
}
