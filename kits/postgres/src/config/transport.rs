// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::IpAddr;
use std::str::FromStr;

use secrecy::{ExposeSecret, SecretString};
use tokio_postgres::config::Host;

use crate::error::{PostgresConfigErrorReason, PostgresConfigField, PostgresResult};

use super::config_error;

/// Plaintext is a local development capability, so every configured target
/// address must remain on this machine even when host and hostaddr are both set.
pub(super) fn validate_plaintext_target(connection_uri: &SecretString) -> PostgresResult<()> {
    let parsed =
        tokio_postgres::Config::from_str(connection_uri.expose_secret()).map_err(|_| {
            config_error(
                PostgresConfigField::ConnectionUri,
                PostgresConfigErrorReason::InvalidSyntax,
            )
        })?;
    let hosts_local = parsed.get_hosts().iter().all(|host| match host {
        Host::Tcp(host) => is_loopback_host(host),
        #[cfg(unix)]
        Host::Unix(_) => true,
    });
    let addresses_local = parsed.get_hostaddrs().iter().all(IpAddr::is_loopback);
    if !hosts_local || !addresses_local {
        return Err(config_error(
            PostgresConfigField::TransportSecurity,
            PostgresConfigErrorReason::Incompatible,
        ));
    }
    Ok(())
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}
