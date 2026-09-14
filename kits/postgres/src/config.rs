// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! PostgreSQL configuration.

use std::path::{Path, PathBuf};
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

use crate::error::{PostgresConfigErrorReason, PostgresConfigField, PostgresError, PostgresResult};

mod env;

use env::{env_name, optional_env, parse_env_u32, parse_env_u64};

const DEFAULT_MAX_POOL_SIZE: u32 = 16;
const DEFAULT_MIN_POOL_SIZE: u32 = 1;
const MAX_POOL_SIZE_UPPER_BOUND: u32 = 512;
const MAX_CONNECTION_URI_BYTES: usize = 16_384;
const MAX_TLS_CA_PATH_BYTES: usize = 4_096;
const DEFAULT_CONNECTION_TIMEOUT_MILLIS: u64 = 5_000;
const MAX_CONNECTION_TIMEOUT_MILLIS: u64 = 300_000;
const MAX_APPLICATION_NAME_BYTES: usize = 63;
const MAX_ENVIRONMENT_PREFIX_BYTES: usize = 128;
const DEFAULT_STATEMENT_TIMEOUT_MILLIS: u64 = 10_000;
const DEFAULT_LOCK_TIMEOUT_MILLIS: u64 = 2_000;
const DEFAULT_IDLE_TRANSACTION_TIMEOUT_MILLIS: u64 = 10_000;
const MAX_QUERY_TIMEOUT_MILLIS: u64 = 300_000;

/// PostgreSQL transport-security policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostgresTransportSecurity {
    /// Require certificate-validated TLS. This is the production default.
    RequireTls,
    /// Permit plaintext only for explicitly configured local development.
    AllowPlaintextForDevelopment,
}

/// Certificate roots trusted by PostgreSQL TLS connections.
#[derive(Clone, PartialEq, Eq)]
pub enum PostgresTlsTrust {
    /// Use the operating system's maintained certificate roots.
    NativeRoots,
    /// Trust only certificates chaining to this private CA PEM file.
    CustomRootCertificate(PathBuf),
}

impl PostgresTlsTrust {
    pub(crate) fn custom_root_certificate(&self) -> Option<&Path> {
        match self {
            Self::NativeRoots => None,
            Self::CustomRootCertificate(path) => Some(path.as_path()),
        }
    }

    const fn mode_name(&self) -> &'static str {
        match self {
            Self::NativeRoots => "native-roots",
            Self::CustomRootCertificate(_) => "custom-root-certificate",
        }
    }
}

impl std::fmt::Debug for PostgresTlsTrust {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.mode_name())
    }
}

/// Raw PostgreSQL configuration input.
pub struct PostgresConfigInput {
    /// PostgreSQL connection URI or keyword connection string.
    pub connection_uri: SecretString,
    /// Optional application name set on PostgreSQL connections.
    pub application_name: Option<String>,
    /// Maximum pooled connections.
    pub max_pool_size: u32,
    /// Minimum idle connections established before pool construction succeeds.
    pub min_pool_size: u32,
    /// Pool connection acquisition timeout.
    pub connection_timeout_millis: u64,
    /// Transport security policy.
    pub transport_security: PostgresTransportSecurity,
    /// Certificate roots used when TLS is required.
    pub tls_trust: PostgresTlsTrust,
    /// Per-statement server-side timeout.
    pub statement_timeout_millis: u64,
    /// Server-side lock acquisition timeout.
    pub lock_timeout_millis: u64,
    /// Server-side idle transaction timeout.
    pub idle_transaction_timeout_millis: u64,
}

impl Default for PostgresConfigInput {
    fn default() -> Self {
        Self {
            connection_uri: SecretString::from(String::new()),
            application_name: None,
            max_pool_size: DEFAULT_MAX_POOL_SIZE,
            min_pool_size: DEFAULT_MIN_POOL_SIZE,
            connection_timeout_millis: DEFAULT_CONNECTION_TIMEOUT_MILLIS,
            transport_security: PostgresTransportSecurity::RequireTls,
            tls_trust: PostgresTlsTrust::NativeRoots,
            statement_timeout_millis: DEFAULT_STATEMENT_TIMEOUT_MILLIS,
            lock_timeout_millis: DEFAULT_LOCK_TIMEOUT_MILLIS,
            idle_transaction_timeout_millis: DEFAULT_IDLE_TRANSACTION_TIMEOUT_MILLIS,
        }
    }
}

impl std::fmt::Debug for PostgresConfigInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresConfigInput")
            .field("connection_uri", &"<redacted>")
            .field("application_name", &self.application_name)
            .field("max_pool_size", &self.max_pool_size)
            .field("min_pool_size", &self.min_pool_size)
            .field("connection_timeout_millis", &self.connection_timeout_millis)
            .field("transport_security", &self.transport_security)
            .field("tls_trust", &self.tls_trust)
            .field("statement_timeout_millis", &self.statement_timeout_millis)
            .field("lock_timeout_millis", &self.lock_timeout_millis)
            .field(
                "idle_transaction_timeout_millis",
                &self.idle_transaction_timeout_millis,
            )
            .finish()
    }
}

/// Validated PostgreSQL configuration.
pub struct PostgresConfig {
    connection_uri: SecretString,
    application_name: Option<String>,
    max_pool_size: u32,
    min_pool_size: u32,
    connection_timeout: Duration,
    transport_security: PostgresTransportSecurity,
    tls_trust: PostgresTlsTrust,
    statement_timeout_millis: u64,
    lock_timeout_millis: u64,
    idle_transaction_timeout_millis: u64,
}

impl PostgresConfig {
    /// Constructs validated PostgreSQL configuration.
    pub fn new(input: PostgresConfigInput) -> PostgresResult<Self> {
        if input.connection_uri.expose_secret().trim().is_empty() {
            return Err(config_error(
                PostgresConfigField::ConnectionUri,
                PostgresConfigErrorReason::Empty,
            ));
        }
        if input.connection_uri.expose_secret().len() > MAX_CONNECTION_URI_BYTES {
            return Err(config_error(
                PostgresConfigField::ConnectionUri,
                PostgresConfigErrorReason::TooLarge,
            ));
        }

        let application_name = input
            .application_name
            .map(validate_application_name)
            .transpose()?;
        validate_positive_bounded_u32(
            input.max_pool_size,
            MAX_POOL_SIZE_UPPER_BOUND,
            PostgresConfigField::MaxPoolSize,
        )?;
        validate_bounded_u32(
            input.min_pool_size,
            input.max_pool_size,
            PostgresConfigField::MinPoolSize,
        )?;
        validate_positive_bounded_u64(
            input.connection_timeout_millis,
            MAX_CONNECTION_TIMEOUT_MILLIS,
            PostgresConfigField::ConnectionTimeoutMillis,
        )?;
        validate_positive_bounded_u64(
            input.statement_timeout_millis,
            MAX_QUERY_TIMEOUT_MILLIS,
            PostgresConfigField::StatementTimeoutMillis,
        )?;
        validate_positive_bounded_u64(
            input.lock_timeout_millis,
            MAX_QUERY_TIMEOUT_MILLIS,
            PostgresConfigField::LockTimeoutMillis,
        )?;
        validate_positive_bounded_u64(
            input.idle_transaction_timeout_millis,
            MAX_QUERY_TIMEOUT_MILLIS,
            PostgresConfigField::IdleTransactionTimeoutMillis,
        )?;
        if input
            .tls_trust
            .custom_root_certificate()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(config_error(
                PostgresConfigField::TlsCaCertificatePath,
                PostgresConfigErrorReason::Empty,
            ));
        }
        if input
            .tls_trust
            .custom_root_certificate()
            .is_some_and(|path| path.as_os_str().as_encoded_bytes().len() > MAX_TLS_CA_PATH_BYTES)
        {
            return Err(config_error(
                PostgresConfigField::TlsCaCertificatePath,
                PostgresConfigErrorReason::TooLarge,
            ));
        }
        if matches!(
            input.transport_security,
            PostgresTransportSecurity::AllowPlaintextForDevelopment
        ) && matches!(input.tls_trust, PostgresTlsTrust::CustomRootCertificate(_))
        {
            return Err(config_error(
                PostgresConfigField::TlsCaCertificatePath,
                PostgresConfigErrorReason::Incompatible,
            ));
        }

        Ok(Self {
            connection_uri: input.connection_uri,
            application_name,
            max_pool_size: input.max_pool_size,
            min_pool_size: input.min_pool_size,
            connection_timeout: Duration::from_millis(input.connection_timeout_millis),
            transport_security: input.transport_security,
            tls_trust: input.tls_trust,
            statement_timeout_millis: input.statement_timeout_millis,
            lock_timeout_millis: input.lock_timeout_millis,
            idle_transaction_timeout_millis: input.idle_transaction_timeout_millis,
        })
    }

    /// Builds configuration from process environment variables using a prefix.
    ///
    /// For `prefix = "EXAMPLE_SERVICE"`, this reads:
    /// `EXAMPLE_SERVICE_POSTGRES_URI`,
    /// `EXAMPLE_SERVICE_POSTGRES_APPLICATION_NAME`,
    /// `EXAMPLE_SERVICE_POSTGRES_MAX_POOL_SIZE`,
    /// `EXAMPLE_SERVICE_POSTGRES_MIN_POOL_SIZE`,
    /// `EXAMPLE_SERVICE_POSTGRES_CONNECTION_TIMEOUT_MILLIS`,
    /// `EXAMPLE_SERVICE_POSTGRES_TLS_MODE`, and the bounded server-side
    /// statement, lock, and idle-transaction timeout variables.
    pub fn from_env_prefix(prefix: &str) -> PostgresResult<Self> {
        let connection_uri_name = env_name(prefix, "POSTGRES_URI")?;
        let application_name_name = env_name(prefix, "POSTGRES_APPLICATION_NAME")?;
        let max_pool_size_name = env_name(prefix, "POSTGRES_MAX_POOL_SIZE")?;
        let min_pool_size_name = env_name(prefix, "POSTGRES_MIN_POOL_SIZE")?;
        let timeout_name = env_name(prefix, "POSTGRES_CONNECTION_TIMEOUT_MILLIS")?;
        let tls_mode_name = env_name(prefix, "POSTGRES_TLS_MODE")?;
        let tls_ca_path_name = env_name(prefix, "POSTGRES_TLS_CA_PEM_PATH")?;
        let statement_timeout_name = env_name(prefix, "POSTGRES_STATEMENT_TIMEOUT_MILLIS")?;
        let lock_timeout_name = env_name(prefix, "POSTGRES_LOCK_TIMEOUT_MILLIS")?;
        let idle_timeout_name = env_name(prefix, "POSTGRES_IDLE_TRANSACTION_TIMEOUT_MILLIS")?;

        let connection_uri = optional_env(connection_uri_name, PostgresConfigField::ConnectionUri)?
            .ok_or_else(|| {
                config_error(
                    PostgresConfigField::ConnectionUri,
                    PostgresConfigErrorReason::Empty,
                )
            })?;
        let application_name =
            optional_env(application_name_name, PostgresConfigField::ApplicationName)?;
        let max_pool_size = parse_env_u32(
            max_pool_size_name,
            DEFAULT_MAX_POOL_SIZE,
            PostgresConfigField::MaxPoolSize,
        )?;
        let min_pool_size = parse_env_u32(
            min_pool_size_name,
            DEFAULT_MIN_POOL_SIZE,
            PostgresConfigField::MinPoolSize,
        )?;
        let connection_timeout_millis = parse_env_u64(
            timeout_name,
            DEFAULT_CONNECTION_TIMEOUT_MILLIS,
            PostgresConfigField::ConnectionTimeoutMillis,
        )?;
        let transport_security =
            match optional_env(tls_mode_name, PostgresConfigField::TransportSecurity)? {
                Some(value) if value == "require" => PostgresTransportSecurity::RequireTls,
                Some(value) if value == "allow-plaintext-development" => {
                    PostgresTransportSecurity::AllowPlaintextForDevelopment
                }
                Some(_) => {
                    return Err(config_error(
                        PostgresConfigField::TransportSecurity,
                        PostgresConfigErrorReason::InvalidSyntax,
                    ));
                }
                None => PostgresTransportSecurity::RequireTls,
            };
        let tls_trust =
            match optional_env(tls_ca_path_name, PostgresConfigField::TlsCaCertificatePath)? {
                Some(value) => PostgresTlsTrust::CustomRootCertificate(PathBuf::from(value)),
                None => PostgresTlsTrust::NativeRoots,
            };
        let statement_timeout_millis = parse_env_u64(
            statement_timeout_name,
            DEFAULT_STATEMENT_TIMEOUT_MILLIS,
            PostgresConfigField::StatementTimeoutMillis,
        )?;
        let lock_timeout_millis = parse_env_u64(
            lock_timeout_name,
            DEFAULT_LOCK_TIMEOUT_MILLIS,
            PostgresConfigField::LockTimeoutMillis,
        )?;
        let idle_transaction_timeout_millis = parse_env_u64(
            idle_timeout_name,
            DEFAULT_IDLE_TRANSACTION_TIMEOUT_MILLIS,
            PostgresConfigField::IdleTransactionTimeoutMillis,
        )?;

        Self::new(PostgresConfigInput {
            connection_uri: SecretString::from(connection_uri),
            application_name,
            max_pool_size,
            min_pool_size,
            connection_timeout_millis,
            transport_security,
            tls_trust,
            statement_timeout_millis,
            lock_timeout_millis,
            idle_transaction_timeout_millis,
        })
    }

    /// Returns the secret connection URI.
    pub(crate) fn connection_uri(&self) -> &SecretString {
        &self.connection_uri
    }

    /// Returns the optional PostgreSQL application name.
    pub fn application_name(&self) -> Option<&str> {
        self.application_name.as_deref()
    }

    /// Returns the maximum pool size.
    pub const fn max_pool_size(&self) -> u32 {
        self.max_pool_size
    }

    /// Returns the minimum idle connections established during startup.
    pub const fn min_pool_size(&self) -> u32 {
        self.min_pool_size
    }

    /// Returns the pool connection timeout.
    pub const fn connection_timeout(&self) -> Duration {
        self.connection_timeout
    }

    /// Returns the transport-security policy.
    pub const fn transport_security(&self) -> PostgresTransportSecurity {
        self.transport_security
    }

    /// Returns the certificate trust policy used for TLS connections.
    pub const fn tls_trust(&self) -> &PostgresTlsTrust {
        &self.tls_trust
    }

    /// Returns bounded PostgreSQL session options applied to every connection.
    pub(crate) fn session_options(&self) -> String {
        format!(
            "-c statement_timeout={} -c lock_timeout={} -c idle_in_transaction_session_timeout={}",
            self.statement_timeout_millis,
            self.lock_timeout_millis,
            self.idle_transaction_timeout_millis,
        )
    }
}

impl std::fmt::Debug for PostgresConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresConfig")
            .field("connection_uri", &"<redacted>")
            .field("application_name", &self.application_name)
            .field("max_pool_size", &self.max_pool_size)
            .field("min_pool_size", &self.min_pool_size)
            .field("connection_timeout", &self.connection_timeout)
            .field("transport_security", &self.transport_security)
            .field("tls_trust", &self.tls_trust)
            .field("statement_timeout_millis", &self.statement_timeout_millis)
            .field("lock_timeout_millis", &self.lock_timeout_millis)
            .field(
                "idle_transaction_timeout_millis",
                &self.idle_transaction_timeout_millis,
            )
            .finish()
    }
}

fn validate_application_name(value: String) -> PostgresResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(config_error(
            PostgresConfigField::ApplicationName,
            PostgresConfigErrorReason::Empty,
        ));
    }
    if trimmed.len() > MAX_APPLICATION_NAME_BYTES {
        return Err(config_error(
            PostgresConfigField::ApplicationName,
            PostgresConfigErrorReason::TooLarge,
        ));
    }
    if !trimmed
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(config_error(
            PostgresConfigField::ApplicationName,
            PostgresConfigErrorReason::InvalidSyntax,
        ));
    }

    Ok(trimmed.to_owned())
}

fn validate_bounded_u32(
    value: u32,
    upper_bound: u32,
    field: PostgresConfigField,
) -> PostgresResult<()> {
    if value > upper_bound {
        return Err(config_error(field, PostgresConfigErrorReason::TooLarge));
    }
    Ok(())
}

fn validate_positive_bounded_u32(
    value: u32,
    upper_bound: u32,
    field: PostgresConfigField,
) -> PostgresResult<()> {
    if value == 0 {
        return Err(config_error(field, PostgresConfigErrorReason::Zero));
    }
    if value > upper_bound {
        return Err(config_error(field, PostgresConfigErrorReason::TooLarge));
    }
    Ok(())
}

fn validate_positive_bounded_u64(
    value: u64,
    upper_bound: u64,
    field: PostgresConfigField,
) -> PostgresResult<()> {
    if value == 0 {
        return Err(config_error(field, PostgresConfigErrorReason::Zero));
    }
    if value > upper_bound {
        return Err(config_error(field, PostgresConfigErrorReason::TooLarge));
    }
    Ok(())
}

fn config_error(field: PostgresConfigField, reason: PostgresConfigErrorReason) -> PostgresError {
    PostgresError::Config { field, reason }
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
