// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::Read;
use std::num::NonZeroU64;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::{
    Client, Cmd, ConnectionAddr, ErrorKind, FromRedisValue, IntoConnectionInfo, Pipeline,
    ProtocolVersion, RedisConnectionInfo, RedisError, TlsCertificates,
};
use rustls::pki_types::pem::PemObject;
use secrecy::ExposeSecret;
use zeroize::Zeroizing;

use crate::{
    ValkeyCommandErrorReason, ValkeyConfig, ValkeyError, ValkeyKey, ValkeyResult,
    ValkeySetupErrorReason, ValkeyTimeToLive, ValkeyTlsTrust, ValkeyTransportSecurity, ValkeyValue,
};

const NAMESPACE_SEPARATOR: u8 = b':';
const RELEASE_LEASE_SCRIPT: &str = "if redis.call('GET', KEYS[1]) == ARGV[1] then return redis.call('DEL', KEYS[1]) else return 0 end";
// Redis integer replies become Lua doubles. Return GET's exact decimal bytes
// so counters above 2^53 retain precision, and test existence before increment
// rather than confusing an existing zero counter with a newly created key.
const INCREMENT_WITH_TTL_SCRIPT: &str = r"
local existed = redis.call('EXISTS', KEYS[1])
redis.call('INCRBY', KEYS[1], ARGV[1])
if existed == 0 then
    redis.call('PEXPIRE', KEYS[1], ARGV[2])
end
return redis.call('GET', KEYS[1])
";
const MAX_TLS_CA_PEM_BYTES: u64 = 1_048_576;
const MAX_TLS_CA_CERTIFICATES: u32 = 64;

/// Low-cardinality readiness snapshot for a Valkey connector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValkeyHealthReport {
    /// Whether certificate-validated TLS is active.
    pub tls_enabled: bool,
    /// Selected logical database number.
    pub database: u32,
    /// Protocol negotiated by the connector.
    pub protocol: ValkeyProtocolVersion,
}

/// Wire protocol selected for managed Valkey connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyProtocolVersion {
    /// RESP3, required by this connector for modern typed responses and push
    /// handling during reconnect detection.
    Resp3,
}

/// Reconnecting, multiplexed Valkey database connector.
///
/// The connector is cheap to clone and shares one bounded connection manager.
/// Its generic query methods discard driver diagnostics and expose only stable
/// kit errors, while typed convenience methods provide common cache, lease,
/// and counter behavior.
#[derive(Clone)]
pub struct ValkeyConnector {
    connection: ConnectionManager,
    key_prefix: Vec<u8>,
    database: u32,
    tls_enabled: bool,
}

impl ValkeyConnector {
    /// Establishes and authenticates the initial connection.
    pub async fn connect(config: &ValkeyConfig) -> ValkeyResult<Self> {
        // Rustls allows only one process-wide crypto provider. A failed install here
        // means another component already selected one, which is a valid composition.
        let _ = rustls::crypto::ring::default_provider().install_default();

        let address = match config.transport_security() {
            ValkeyTransportSecurity::RequireTls => ConnectionAddr::TcpTls {
                host: config.host().to_owned(),
                port: config.port(),
                insecure: false,
                tls_params: None,
            },
            ValkeyTransportSecurity::AllowPlaintextForDevelopment => {
                ConnectionAddr::Tcp(config.host().to_owned(), config.port())
            }
        };
        if !address.is_supported() {
            return Err(setup_error(ValkeySetupErrorReason::TlsProviderUnavailable));
        }

        let database = i64::from(config.database());
        let mut settings = RedisConnectionInfo::default()
            .set_db(database)
            .set_protocol(ProtocolVersion::RESP3)
            .set_lib_name("reallyme-valkey-kit", env!("CARGO_PKG_VERSION"));
        if let Some(username) = config.username() {
            settings = settings.set_username(username.expose_secret());
        }
        if let Some(password) = config.password() {
            settings = settings.set_password(password.expose_secret());
        }

        let connection_info = (config.host(), config.port())
            .into_connection_info()
            .map_err(map_setup_error)?
            .set_addr(address)
            .set_redis_settings(settings);
        let client = configured_client(config, connection_info)?;
        let retry_attempts = usize::try_from(config.retry_attempts())
            .map_err(|_| setup_error(ValkeySetupErrorReason::InvalidEndpoint))?;
        let concurrency_limit = usize::try_from(config.concurrency_limit())
            .map_err(|_| setup_error(ValkeySetupErrorReason::InvalidEndpoint))?;
        let pipeline_buffer_size = usize::try_from(config.pipeline_buffer_size())
            .map_err(|_| setup_error(ValkeySetupErrorReason::InvalidEndpoint))?;
        let manager_config = ConnectionManagerConfig::new()
            .set_number_of_retries(retry_attempts)
            .set_connection_timeout(Some(config.connection_timeout()))
            .set_response_timeout(Some(config.response_timeout()))
            .set_concurrency_limit(concurrency_limit)
            .set_pipeline_buffer_size(pipeline_buffer_size);
        let connection = client
            .get_connection_manager_with_config(manager_config)
            .await
            .map_err(map_setup_error)?;

        Ok(Self {
            connection,
            key_prefix: config.key_prefix().as_bytes().to_vec(),
            database: config.database(),
            tls_enabled: matches!(
                config.transport_security(),
                ValkeyTransportSecurity::RequireTls
            ),
        })
    }

    /// Verifies that the server accepts commands on the current connection.
    pub async fn health_check(&self) -> ValkeyResult<()> {
        self.health_report().await.map(|_report| ())
    }

    /// Verifies readiness and returns bounded, non-sensitive connector state.
    pub async fn health_report(&self) -> ValkeyResult<ValkeyHealthReport> {
        let mut connection = self.connection.clone();
        let response: String = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)?;
        if response != "PONG" {
            return Err(command_error(ValkeyCommandErrorReason::InvalidResponse));
        }
        Ok(ValkeyHealthReport {
            tls_enabled: self.tls_enabled,
            database: self.database,
            protocol: ValkeyProtocolVersion::Resp3,
        })
    }

    /// Executes an app-owned command through the bounded managed connection.
    ///
    /// Command names must be fixed by application code. Untrusted data belongs
    /// only in encoded command arguments, never in the command name. Apps that
    /// use keys should call [`Self::namespaced_key`] so deployments retain
    /// namespace isolation. Raw driver diagnostics are discarded at this
    /// boundary and never become application errors or logs.
    pub async fn query<T>(&self, command: &Cmd) -> ValkeyResult<T>
    where
        T: FromRedisValue,
    {
        let mut connection = self.connection.clone();
        command
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)
    }

    /// Executes an app-owned pipeline through the bounded managed connection.
    ///
    /// A pipeline is not atomic unless the caller explicitly enables
    /// transaction mode on the pipeline. The same namespace and untrusted-input
    /// requirements as [`Self::query`] apply.
    pub async fn query_pipeline<T>(&self, pipeline: &Pipeline) -> ValkeyResult<T>
    where
        T: FromRedisValue,
    {
        let mut connection = self.connection.clone();
        pipeline
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)
    }

    /// Reads a value, returning `None` when the key is absent or expired.
    pub async fn get(&self, key: &ValkeyKey) -> ValkeyResult<Option<ValkeyValue>> {
        let namespaced_key = self.namespaced_key(key)?;
        let mut connection = self.connection.clone();
        let response: Option<Vec<u8>> = redis::cmd("GET")
            .arg(namespaced_key.as_slice())
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)?;
        response.map(ValkeyValue::new).transpose()
    }

    /// Stores a value with a mandatory expiration.
    pub async fn set_with_ttl(
        &self,
        key: &ValkeyKey,
        value: &ValkeyValue,
        ttl: ValkeyTimeToLive,
    ) -> ValkeyResult<()> {
        let namespaced_key = self.namespaced_key(key)?;
        let mut connection = self.connection.clone();
        redis::cmd("SET")
            .arg(namespaced_key.as_slice())
            .arg(value.as_bytes())
            .arg("PX")
            .arg(ttl.milliseconds())
            .query_async::<()>(&mut connection)
            .await
            .map_err(map_command_error)
    }

    /// Acquires a lease only when its key is absent, always with an expiration.
    pub async fn set_if_absent_with_ttl(
        &self,
        key: &ValkeyKey,
        value: &ValkeyValue,
        ttl: ValkeyTimeToLive,
    ) -> ValkeyResult<bool> {
        let namespaced_key = self.namespaced_key(key)?;
        let mut connection = self.connection.clone();
        let response: Option<()> = redis::cmd("SET")
            .arg(namespaced_key.as_slice())
            .arg(value.as_bytes())
            .arg("PX")
            .arg(ttl.milliseconds())
            .arg("NX")
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)?;
        Ok(response.is_some())
    }

    /// Deletes a key and reports whether it existed.
    pub async fn delete(&self, key: &ValkeyKey) -> ValkeyResult<bool> {
        let namespaced_key = self.namespaced_key(key)?;
        let mut connection = self.connection.clone();
        let deleted: u64 = redis::cmd("DEL")
            .arg(namespaced_key.as_slice())
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)?;
        decode_deleted_count(deleted)
    }

    /// Atomically releases a lease only when the stored token still matches.
    pub async fn release_lease(
        &self,
        key: &ValkeyKey,
        lease_token: &ValkeyValue,
    ) -> ValkeyResult<bool> {
        let namespaced_key = self.namespaced_key(key)?;
        let mut connection = self.connection.clone();
        let deleted: u64 = redis::cmd("EVAL")
            .arg(RELEASE_LEASE_SCRIPT)
            .arg(1_u8)
            .arg(namespaced_key.as_slice())
            .arg(lease_token.as_bytes())
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)?;
        decode_deleted_count(deleted)
    }

    /// Atomically increments a counter and sets its TTL only when first created.
    pub async fn increment_with_ttl(
        &self,
        key: &ValkeyKey,
        increment: NonZeroU64,
        ttl: ValkeyTimeToLive,
    ) -> ValkeyResult<u64> {
        let namespaced_key = self.namespaced_key(key)?;
        let mut connection = self.connection.clone();
        redis::cmd("EVAL")
            .arg(INCREMENT_WITH_TTL_SCRIPT)
            .arg(1_u8)
            .arg(namespaced_key.as_slice())
            .arg(increment.get())
            .arg(ttl.milliseconds())
            .query_async(&mut connection)
            .await
            .map_err(map_command_error)
    }

    /// Builds the deployment namespace plus a validated application key.
    ///
    /// The returned buffer clears its allocation on drop and can be passed
    /// directly to a Valkey command as binary data.
    pub fn namespaced_key(&self, key: &ValkeyKey) -> ValkeyResult<Zeroizing<Vec<u8>>> {
        let with_separator = self
            .key_prefix
            .len()
            .checked_add(1)
            .ok_or_else(|| command_error(ValkeyCommandErrorReason::Rejected))?;
        let capacity = with_separator
            .checked_add(key.as_bytes().len())
            .ok_or_else(|| command_error(ValkeyCommandErrorReason::Rejected))?;
        let mut value = Zeroizing::new(Vec::with_capacity(capacity));
        value.extend_from_slice(self.key_prefix.as_slice());
        value.push(NAMESPACE_SEPARATOR);
        value.extend_from_slice(key.as_bytes());
        Ok(value)
    }
}

fn configured_client(
    config: &ValkeyConfig,
    connection_info: redis::ConnectionInfo,
) -> ValkeyResult<Client> {
    match config.tls_trust() {
        ValkeyTlsTrust::WebPkiRoots => Client::open(connection_info).map_err(map_setup_error),
        ValkeyTlsTrust::CustomRootCertificate(_) => {
            let path = config
                .tls_trust()
                .custom_root_certificate()
                .ok_or_else(|| setup_error(ValkeySetupErrorReason::TlsTrustUnavailable))?;
            let root_cert = read_custom_root(path)?;
            Client::build_with_tls(
                connection_info,
                TlsCertificates {
                    client_tls: None,
                    root_cert: Some(root_cert),
                },
            )
            .map_err(|_error| setup_error(ValkeySetupErrorReason::TlsTrustInvalid))
        }
    }
}

fn read_custom_root(path: &std::path::Path) -> ValkeyResult<Vec<u8>> {
    let file = std::fs::File::open(path)
        .map_err(|_error| setup_error(ValkeySetupErrorReason::TlsTrustUnavailable))?;
    let metadata = file
        .metadata()
        .map_err(|_error| setup_error(ValkeySetupErrorReason::TlsTrustUnavailable))?;
    if !metadata.is_file() {
        return Err(setup_error(ValkeySetupErrorReason::TlsTrustInvalid));
    }
    if metadata.len() > MAX_TLS_CA_PEM_BYTES {
        return Err(setup_error(ValkeySetupErrorReason::TlsTrustTooLarge));
    }

    // Metadata is not a sufficient allocation boundary because an
    // administrator-provided file may be replaced between stat and read.
    let read_limit = MAX_TLS_CA_PEM_BYTES
        .checked_add(1)
        .ok_or_else(|| setup_error(ValkeySetupErrorReason::TlsTrustTooLarge))?;
    let mut pem = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut pem)
        .map_err(|_error| setup_error(ValkeySetupErrorReason::TlsTrustUnavailable))?;
    let length = u64::try_from(pem.len())
        .map_err(|_error| setup_error(ValkeySetupErrorReason::TlsTrustTooLarge))?;
    if length > MAX_TLS_CA_PEM_BYTES {
        return Err(setup_error(ValkeySetupErrorReason::TlsTrustTooLarge));
    }
    if pem.is_empty() {
        return Err(setup_error(ValkeySetupErrorReason::TlsTrustInvalid));
    }
    let mut certificate_count = 0_u32;
    for certificate in rustls::pki_types::CertificateDer::pem_slice_iter(&pem) {
        let _certificate =
            certificate.map_err(|_error| setup_error(ValkeySetupErrorReason::TlsTrustInvalid))?;
        certificate_count = certificate_count
            .checked_add(1)
            .ok_or_else(|| setup_error(ValkeySetupErrorReason::TlsTrustTooLarge))?;
        if certificate_count > MAX_TLS_CA_CERTIFICATES {
            return Err(setup_error(ValkeySetupErrorReason::TlsTrustTooLarge));
        }
    }
    if certificate_count == 0 {
        return Err(setup_error(ValkeySetupErrorReason::TlsTrustInvalid));
    }
    Ok(pem)
}

impl std::fmt::Debug for ValkeyConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValkeyConnector")
            .field("connection", &"<managed>")
            .field("key_prefix", &"<redacted>")
            .finish()
    }
}

fn decode_deleted_count(value: u64) -> ValkeyResult<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(command_error(ValkeyCommandErrorReason::InvalidResponse)),
    }
}

fn map_setup_error(error: RedisError) -> ValkeyError {
    let reason = match error.kind() {
        ErrorKind::InvalidClientConfig => ValkeySetupErrorReason::InvalidEndpoint,
        ErrorKind::AuthenticationFailed => ValkeySetupErrorReason::AuthenticationRejected,
        _ => ValkeySetupErrorReason::ConnectionUnavailable,
    };
    setup_error(reason)
}

fn map_command_error(error: RedisError) -> ValkeyError {
    let reason = if error.is_timeout() {
        ValkeyCommandErrorReason::Timeout
    } else if error.is_io_error() || error.is_connection_dropped() {
        ValkeyCommandErrorReason::ConnectionUnavailable
    } else if matches!(
        error.kind(),
        ErrorKind::Parse | ErrorKind::UnexpectedReturnType | ErrorKind::RESP3NotSupported
    ) {
        ValkeyCommandErrorReason::InvalidResponse
    } else {
        ValkeyCommandErrorReason::Rejected
    };
    command_error(reason)
}

const fn setup_error(reason: ValkeySetupErrorReason) -> ValkeyError {
    ValkeyError::Setup { reason }
}

const fn command_error(reason: ValkeyCommandErrorReason) -> ValkeyError {
    ValkeyError::Command { reason }
}

#[cfg(test)]
mod tests {
    use super::{MAX_TLS_CA_PEM_BYTES, decode_deleted_count, read_custom_root};
    use crate::{ValkeyCommandErrorReason, ValkeyError, ValkeySetupErrorReason};

    #[test]
    fn delete_count_rejects_protocol_violation() {
        assert_eq!(decode_deleted_count(0), Ok(false));
        assert_eq!(decode_deleted_count(1), Ok(true));
        assert_eq!(
            decode_deleted_count(2),
            Err(ValkeyError::Command {
                reason: ValkeyCommandErrorReason::InvalidResponse,
            })
        );
    }

    #[test]
    fn custom_root_reader_rejects_missing_empty_and_oversized_files() {
        let directory = tempfile::tempdir().expect("temporary directory should be available");
        let missing = directory.path().join("missing.pem");
        assert_eq!(
            read_custom_root(&missing).err(),
            Some(ValkeyError::Setup {
                reason: ValkeySetupErrorReason::TlsTrustUnavailable,
            })
        );

        let empty = directory.path().join("empty.pem");
        std::fs::write(&empty, []).expect("empty fixture should be written");
        assert_eq!(
            read_custom_root(&empty).err(),
            Some(ValkeyError::Setup {
                reason: ValkeySetupErrorReason::TlsTrustInvalid,
            })
        );

        let oversized = directory.path().join("oversized.pem");
        let file = std::fs::File::create(&oversized).expect("oversized fixture should be created");
        file.set_len(MAX_TLS_CA_PEM_BYTES + 1)
            .expect("oversized fixture should be sized");
        assert_eq!(
            read_custom_root(&oversized).err(),
            Some(ValkeyError::Setup {
                reason: ValkeySetupErrorReason::TlsTrustTooLarge,
            })
        );
    }
}
