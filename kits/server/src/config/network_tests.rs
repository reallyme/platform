// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::{Ipv4Addr, SocketAddrV4};

use super::{BindAddress, NetworkPort};
use crate::config::{BindAddressConfigField, ConfigError, ConfigValidationErrorReason};

#[test]
fn network_port_rejects_zero() {
    let result = NetworkPort::new(0);

    assert_eq!(
        result,
        Err(ConfigError::InvalidBindAddressConfig {
            field: BindAddressConfigField::Port,
            reason: ConfigValidationErrorReason::MustBeGreaterThanZero,
        })
    );
}

#[test]
fn bind_address_port_returns_validated_network_port() {
    let socket_address = std::net::SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8443));
    let bind_address = BindAddress::new(socket_address);

    assert_eq!(
        bind_address.map(|address| address.port().as_u16()),
        Ok(8443)
    );
}
