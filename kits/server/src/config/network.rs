// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{IpAddr, SocketAddr};

use super::error::{BindAddressConfigField, ConfigError, ConfigValidationErrorReason};

/// Validated network port.
///
/// # Examples
///
/// ```rust
/// use reallyme_server_kit::config::NetworkPort;
///
/// let port = NetworkPort::new(8443)?;
///
/// assert_eq!(port.as_u16(), 8443);
/// # Ok::<(), reallyme_server_kit::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetworkPort(u16);

impl NetworkPort {
    /// Constructs a validated network port.
    ///
    /// Port `0` is rejected intentionally for server runtime configuration so
    /// startup fails closed instead of silently binding to an OS-assigned
    /// ephemeral port. If the platform later needs an explicit ephemeral-port
    /// mode for tests or developer tooling, that should be modeled with a
    /// separate abstraction rather than weakening this production-facing type.
    pub fn new(value: u16) -> Result<Self, ConfigError> {
        if value == 0 {
            return Err(ConfigError::InvalidBindAddressConfig {
                field: BindAddressConfigField::Port,
                reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        Ok(Self(value))
    }

    /// Constructs a port value from a caller-proven invariant.
    ///
    /// This is kept private so only code that has already established the
    /// non-zero invariant can bypass revalidation in an auditable way.
    fn new_unchecked(value: u16) -> Self {
        Self(value)
    }

    /// Returns the port number.
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

/// Validated network bind address.
///
/// # Examples
///
/// ```rust
/// use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
///
/// use reallyme_server_kit::config::BindAddress;
///
/// let bind_address =
///     BindAddress::new(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080)))?;
///
/// assert_eq!(bind_address.port().as_u16(), 8080);
/// assert_eq!(bind_address.ip_addr(), IpAddr::V4(Ipv4Addr::LOCALHOST));
/// # Ok::<(), reallyme_server_kit::config::ConfigError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindAddress(SocketAddr);

impl BindAddress {
    /// Constructs a validated bind address.
    pub fn new(value: SocketAddr) -> Result<Self, ConfigError> {
        let _ = NetworkPort::new(value.port())?;
        Ok(Self(value))
    }

    /// Returns the address as a socket address.
    pub fn as_socket_addr(self) -> SocketAddr {
        self.0
    }

    /// Returns the bound IP address.
    pub fn ip_addr(self) -> IpAddr {
        self.0.ip()
    }

    /// Returns the validated bound port.
    pub fn port(self) -> NetworkPort {
        // `BindAddress::new` validates the socket address port before storing
        // it, so the embedded port is already known to satisfy the
        // `NetworkPort` invariant here.
        NetworkPort::new_unchecked(self.0.port())
    }
}

#[cfg(test)]
#[path = "network_tests.rs"]
mod tests;
