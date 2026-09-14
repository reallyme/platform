// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};
use std::time::Duration;

use secrecy::SecretString;

use crate::{ValkeyConfigErrorReason, ValkeyConfigField, ValkeyError, ValkeyResult};

mod env;
mod validation;

use env::{env_name, optional_env, parse_env_u16, parse_env_u32, parse_env_u64, required_env};
use validation::{
    validate_host, validate_key_prefix, validate_optional_secret, validate_tls_trust,
};

const DEFAULT_PORT: u16 = 6_379;
const DEFAULT_DATABASE: u32 = 0;
const DEFAULT_CONNECTION_TIMEOUT_MILLIS: u64 = 3_000;
const DEFAULT_RESPONSE_TIMEOUT_MILLIS: u64 = 2_000;
const DEFAULT_RETRY_ATTEMPTS: u32 = 3;
const DEFAULT_CONCURRENCY_LIMIT: u32 = 1_024;
const DEFAULT_PIPELINE_BUFFER_SIZE: u32 = 256;
const MAX_HOST_BYTES: usize = 253;
const MAX_DATABASE: u32 = 1_023;
const MAX_KEY_PREFIX_BYTES: usize = 64;
const MAX_CREDENTIAL_BYTES: usize = 4_096;
const MAX_ENVIRONMENT_PREFIX_BYTES: usize = 128;
const MAX_TLS_CA_PATH_BYTES: usize = 4_096;
const MAX_TIMEOUT_MILLIS: u64 = 60_000;
const MAX_RETRY_ATTEMPTS: u32 = 20;
const MAX_CONCURRENCY_LIMIT: u32 = 65_536;
const MAX_PIPELINE_BUFFER_SIZE: u32 = 65_536;

/// Valkey transport security policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyTransportSecurity {
    /// Require certificate-validated TLS.
    RequireTls,
    /// Permit plaintext only in explicitly selected development composition.
    AllowPlaintextForDevelopment,
}

/// Certificate roots trusted by Valkey TLS connections.
#[derive(Clone, PartialEq, Eq)]
pub enum ValkeyTlsTrust {
    /// Use the maintained public roots compiled into the Valkey client.
    WebPkiRoots,
    /// Trust only certificates chaining to this private CA PEM file.
    CustomRootCertificate(PathBuf),
}

impl ValkeyTlsTrust {
    pub(crate) fn custom_root_certificate(&self) -> Option<&Path> {
        match self {
            Self::WebPkiRoots => None,
            Self::CustomRootCertificate(path) => Some(path.as_path()),
        }
    }

    const fn mode_name(&self) -> &'static str {
        match self {
            Self::WebPkiRoots => "webpki-roots",
            Self::CustomRootCertificate(_) => "custom-root-certificate",
        }
    }
}

impl std::fmt::Debug for ValkeyTlsTrust {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.mode_name())
    }
}

/// Raw Valkey configuration input.
pub struct ValkeyConfigInput {
    /// DNS hostname or IP address without a URI scheme.
    pub host: String,
    /// TCP port.
    pub port: u16,
    /// Logical database number.
    pub database: u32,
    /// Optional ACL username, treated as sensitive operational metadata.
    pub username: Option<SecretString>,
    /// Optional password or access token.
    pub password: Option<SecretString>,
    /// Prefix prepended to every binary key.
    pub key_prefix: String,
    /// Transport security policy.
    pub transport_security: ValkeyTransportSecurity,
    /// Certificate roots used when TLS is required.
    pub tls_trust: ValkeyTlsTrust,
    /// Connection establishment deadline.
    pub connection_timeout_millis: u64,
    /// Per-command response deadline.
    pub response_timeout_millis: u64,
    /// Automatic reconnect attempt count.
    pub retry_attempts: u32,
    /// Concurrent in-flight command ceiling.
    pub concurrency_limit: u32,
    /// Bounded outbound pipeline queue size.
    pub pipeline_buffer_size: u32,
}

impl Default for ValkeyConfigInput {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: DEFAULT_PORT,
            database: DEFAULT_DATABASE,
            username: None,
            password: None,
            key_prefix: "reallyme".to_owned(),
            transport_security: ValkeyTransportSecurity::RequireTls,
            tls_trust: ValkeyTlsTrust::WebPkiRoots,
            connection_timeout_millis: DEFAULT_CONNECTION_TIMEOUT_MILLIS,
            response_timeout_millis: DEFAULT_RESPONSE_TIMEOUT_MILLIS,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            concurrency_limit: DEFAULT_CONCURRENCY_LIMIT,
            pipeline_buffer_size: DEFAULT_PIPELINE_BUFFER_SIZE,
        }
    }
}

impl std::fmt::Debug for ValkeyConfigInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValkeyConfigInput")
            .field("host", &"<redacted-endpoint>")
            .field("port", &self.port)
            .field("database", &self.database)
            .field("username", &self.username.as_ref().map(|_| "<redacted>"))
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("key_prefix", &self.key_prefix)
            .field("transport_security", &self.transport_security)
            .field("tls_trust", &self.tls_trust)
            .field("connection_timeout_millis", &self.connection_timeout_millis)
            .field("response_timeout_millis", &self.response_timeout_millis)
            .field("retry_attempts", &self.retry_attempts)
            .field("concurrency_limit", &self.concurrency_limit)
            .field("pipeline_buffer_size", &self.pipeline_buffer_size)
            .finish()
    }
}

/// Validated Valkey client configuration.
pub struct ValkeyConfig {
    host: String,
    port: u16,
    database: u32,
    username: Option<SecretString>,
    password: Option<SecretString>,
    key_prefix: String,
    transport_security: ValkeyTransportSecurity,
    tls_trust: ValkeyTlsTrust,
    connection_timeout: Duration,
    response_timeout: Duration,
    retry_attempts: u32,
    concurrency_limit: u32,
    pipeline_buffer_size: u32,
}

impl ValkeyConfig {
    /// Validates and constructs Valkey configuration.
    pub fn new(input: ValkeyConfigInput) -> ValkeyResult<Self> {
        validate_host(input.host.as_str())?;
        if input.port == 0 {
            return Err(config_error(
                ValkeyConfigField::Port,
                ValkeyConfigErrorReason::MustBePositive,
            ));
        }
        if input.database > MAX_DATABASE {
            return Err(config_error(
                ValkeyConfigField::Database,
                ValkeyConfigErrorReason::TooLarge,
            ));
        }
        validate_key_prefix(input.key_prefix.as_str())?;
        validate_optional_secret(input.username.as_ref(), ValkeyConfigField::Username)?;
        validate_optional_secret(input.password.as_ref(), ValkeyConfigField::Password)?;
        validate_positive_bounded_u64(
            input.connection_timeout_millis,
            MAX_TIMEOUT_MILLIS,
            ValkeyConfigField::ConnectionTimeout,
        )?;
        validate_positive_bounded_u64(
            input.response_timeout_millis,
            MAX_TIMEOUT_MILLIS,
            ValkeyConfigField::ResponseTimeout,
        )?;
        validate_bounded_u32(
            input.retry_attempts,
            MAX_RETRY_ATTEMPTS,
            ValkeyConfigField::RetryAttempts,
            false,
        )?;
        validate_bounded_u32(
            input.concurrency_limit,
            MAX_CONCURRENCY_LIMIT,
            ValkeyConfigField::ConcurrencyLimit,
            true,
        )?;
        validate_bounded_u32(
            input.pipeline_buffer_size,
            MAX_PIPELINE_BUFFER_SIZE,
            ValkeyConfigField::PipelineBufferSize,
            true,
        )?;
        validate_tls_trust(&input.tls_trust)?;
        if matches!(
            input.transport_security,
            ValkeyTransportSecurity::AllowPlaintextForDevelopment
        ) && matches!(input.tls_trust, ValkeyTlsTrust::CustomRootCertificate(_))
        {
            return Err(config_error(
                ValkeyConfigField::TlsCaCertificatePath,
                ValkeyConfigErrorReason::Incompatible,
            ));
        }

        Ok(Self {
            host: input.host,
            port: input.port,
            database: input.database,
            username: input.username,
            password: input.password,
            key_prefix: input.key_prefix,
            transport_security: input.transport_security,
            tls_trust: input.tls_trust,
            connection_timeout: Duration::from_millis(input.connection_timeout_millis),
            response_timeout: Duration::from_millis(input.response_timeout_millis),
            retry_attempts: input.retry_attempts,
            concurrency_limit: input.concurrency_limit,
            pipeline_buffer_size: input.pipeline_buffer_size,
        })
    }

    /// Builds configuration from process environment variables using a prefix.
    ///
    /// For `prefix = "EXAMPLE_SEARCH"`, this reads the required
    /// `EXAMPLE_SEARCH_VALKEY_HOST` and optional `VALKEY_PORT`,
    /// `VALKEY_DATABASE`, `VALKEY_USERNAME`, `VALKEY_PASSWORD`,
    /// `VALKEY_KEY_PREFIX`, `VALKEY_TLS_MODE`, and bounded connection-manager
    /// policy variables with the same prefix. TLS defaults to required and
    /// plaintext requires the exact `allow-plaintext-development` value.
    pub fn from_env_prefix(prefix: &str) -> ValkeyResult<Self> {
        let host = required_env(env_name(prefix, "VALKEY_HOST")?, ValkeyConfigField::Host)?;
        let port = parse_env_u16(
            env_name(prefix, "VALKEY_PORT")?,
            DEFAULT_PORT,
            ValkeyConfigField::Port,
        )?;
        let database = parse_env_u32(
            env_name(prefix, "VALKEY_DATABASE")?,
            DEFAULT_DATABASE,
            ValkeyConfigField::Database,
        )?;
        let username = optional_env(
            env_name(prefix, "VALKEY_USERNAME")?,
            ValkeyConfigField::Username,
        )?
        .map(SecretString::from);
        let password = optional_env(
            env_name(prefix, "VALKEY_PASSWORD")?,
            ValkeyConfigField::Password,
        )?
        .map(SecretString::from);
        let key_prefix = optional_env(
            env_name(prefix, "VALKEY_KEY_PREFIX")?,
            ValkeyConfigField::KeyPrefix,
        )?
        .unwrap_or_else(|| "reallyme".to_owned());
        let transport_security = match optional_env(
            env_name(prefix, "VALKEY_TLS_MODE")?,
            ValkeyConfigField::TransportSecurity,
        )? {
            Some(value) if value == "require" => ValkeyTransportSecurity::RequireTls,
            Some(value) if value == "allow-plaintext-development" => {
                ValkeyTransportSecurity::AllowPlaintextForDevelopment
            }
            Some(_) => {
                return Err(config_error(
                    ValkeyConfigField::TransportSecurity,
                    ValkeyConfigErrorReason::InvalidSyntax,
                ));
            }
            None => ValkeyTransportSecurity::RequireTls,
        };
        let tls_trust = match optional_env(
            env_name(prefix, "VALKEY_TLS_CA_PEM_PATH")?,
            ValkeyConfigField::TlsCaCertificatePath,
        )? {
            Some(value) => ValkeyTlsTrust::CustomRootCertificate(PathBuf::from(value)),
            None => ValkeyTlsTrust::WebPkiRoots,
        };
        let connection_timeout_millis = parse_env_u64(
            env_name(prefix, "VALKEY_CONNECTION_TIMEOUT_MILLIS")?,
            DEFAULT_CONNECTION_TIMEOUT_MILLIS,
            ValkeyConfigField::ConnectionTimeout,
        )?;
        let response_timeout_millis = parse_env_u64(
            env_name(prefix, "VALKEY_RESPONSE_TIMEOUT_MILLIS")?,
            DEFAULT_RESPONSE_TIMEOUT_MILLIS,
            ValkeyConfigField::ResponseTimeout,
        )?;
        let retry_attempts = parse_env_u32(
            env_name(prefix, "VALKEY_RETRY_ATTEMPTS")?,
            DEFAULT_RETRY_ATTEMPTS,
            ValkeyConfigField::RetryAttempts,
        )?;
        let concurrency_limit = parse_env_u32(
            env_name(prefix, "VALKEY_CONCURRENCY_LIMIT")?,
            DEFAULT_CONCURRENCY_LIMIT,
            ValkeyConfigField::ConcurrencyLimit,
        )?;
        let pipeline_buffer_size = parse_env_u32(
            env_name(prefix, "VALKEY_PIPELINE_BUFFER_SIZE")?,
            DEFAULT_PIPELINE_BUFFER_SIZE,
            ValkeyConfigField::PipelineBufferSize,
        )?;

        Self::new(ValkeyConfigInput {
            host,
            port,
            database,
            username,
            password,
            key_prefix,
            transport_security,
            tls_trust,
            connection_timeout_millis,
            response_timeout_millis,
            retry_attempts,
            concurrency_limit,
            pipeline_buffer_size,
        })
    }

    /// Returns the server hostname or IP address.
    pub fn host(&self) -> &str {
        self.host.as_str()
    }
    /// Returns the server TCP port.
    pub const fn port(&self) -> u16 {
        self.port
    }
    /// Returns the logical database number.
    pub const fn database(&self) -> u32 {
        self.database
    }
    /// Returns the optional ACL username.
    pub const fn username(&self) -> Option<&SecretString> {
        self.username.as_ref()
    }
    /// Returns the optional password or access token.
    pub const fn password(&self) -> Option<&SecretString> {
        self.password.as_ref()
    }
    /// Returns the key namespace prefix.
    pub fn key_prefix(&self) -> &str {
        self.key_prefix.as_str()
    }
    /// Returns the transport security policy.
    pub const fn transport_security(&self) -> ValkeyTransportSecurity {
        self.transport_security
    }
    /// Returns the certificate trust policy used for TLS connections.
    pub const fn tls_trust(&self) -> &ValkeyTlsTrust {
        &self.tls_trust
    }
    /// Returns the connection establishment deadline.
    pub const fn connection_timeout(&self) -> Duration {
        self.connection_timeout
    }
    /// Returns the command response deadline.
    pub const fn response_timeout(&self) -> Duration {
        self.response_timeout
    }
    /// Returns automatic reconnect attempts.
    pub const fn retry_attempts(&self) -> u32 {
        self.retry_attempts
    }
    /// Returns the in-flight command ceiling.
    pub const fn concurrency_limit(&self) -> u32 {
        self.concurrency_limit
    }
    /// Returns the outbound pipeline queue bound.
    pub const fn pipeline_buffer_size(&self) -> u32 {
        self.pipeline_buffer_size
    }
}

impl std::fmt::Debug for ValkeyConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValkeyConfig")
            .field("host", &"<redacted-endpoint>")
            .field("port", &self.port)
            .field("database", &self.database)
            .field("username", &self.username.as_ref().map(|_| "<redacted>"))
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .field("key_prefix", &self.key_prefix)
            .field("transport_security", &self.transport_security)
            .field("tls_trust", &self.tls_trust)
            .field("connection_timeout", &self.connection_timeout)
            .field("response_timeout", &self.response_timeout)
            .field("retry_attempts", &self.retry_attempts)
            .field("concurrency_limit", &self.concurrency_limit)
            .field("pipeline_buffer_size", &self.pipeline_buffer_size)
            .finish()
    }
}

fn validate_positive_bounded_u64(
    value: u64,
    maximum: u64,
    field: ValkeyConfigField,
) -> ValkeyResult<()> {
    if value == 0 {
        return Err(config_error(field, ValkeyConfigErrorReason::MustBePositive));
    }
    if value > maximum {
        return Err(config_error(field, ValkeyConfigErrorReason::TooLarge));
    }
    Ok(())
}

fn validate_bounded_u32(
    value: u32,
    maximum: u32,
    field: ValkeyConfigField,
    positive: bool,
) -> ValkeyResult<()> {
    if positive && value == 0 {
        return Err(config_error(field, ValkeyConfigErrorReason::MustBePositive));
    }
    if value > maximum {
        return Err(config_error(field, ValkeyConfigErrorReason::TooLarge));
    }
    Ok(())
}

const fn config_error(field: ValkeyConfigField, reason: ValkeyConfigErrorReason) -> ValkeyError {
    ValkeyError::Config { field, reason }
}

#[cfg(test)]
#[path = "config/tests.rs"]
mod tests;
