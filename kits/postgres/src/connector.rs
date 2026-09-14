// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! PostgreSQL pooled connector.

use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use bb8::PooledConnection;
use bb8_postgres::PostgresConnectionManager;
use rustls::pki_types::pem::PemObject;
use secrecy::ExposeSecret;
use tokio_postgres::config::SslMode;
use tokio_postgres::{Client, Config as TokioPostgresConfig, NoTls};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::config::{PostgresConfig, PostgresTlsTrust, PostgresTransportSecurity};
use crate::error::{
    PostgresError, PostgresQueryErrorReason, PostgresResult, PostgresSetupErrorReason,
};

type TlsManager = PostgresConnectionManager<MakeRustlsConnect>;
type PlaintextManager = PostgresConnectionManager<NoTls>;
const MAX_TLS_CA_PEM_BYTES: u64 = 1_048_576;
const MAX_TLS_CA_CERTIFICATES: u32 = 64;

enum PostgresPoolInner {
    Tls(bb8::Pool<TlsManager>),
    PlaintextDevelopment(bb8::Pool<PlaintextManager>),
}

enum PostgresPooledConnectionInner<'a> {
    Tls(PooledConnection<'a, TlsManager>),
    PlaintextDevelopment(PooledConnection<'a, PlaintextManager>),
}

/// Pooled PostgreSQL connection independent of the selected TLS transport.
pub struct PostgresPooledConnection<'a> {
    inner: PostgresPooledConnectionInner<'a>,
}

impl Deref for PostgresPooledConnection<'_> {
    type Target = Client;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            PostgresPooledConnectionInner::Tls(value) => value,
            PostgresPooledConnectionInner::PlaintextDevelopment(value) => value,
        }
    }
}

impl DerefMut for PostgresPooledConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.inner {
            PostgresPooledConnectionInner::Tls(value) => value,
            PostgresPooledConnectionInner::PlaintextDevelopment(value) => value,
        }
    }
}

impl std::fmt::Debug for PostgresPooledConnection<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PostgresPooledConnection(<redacted>)")
    }
}

/// Low-cardinality PostgreSQL pool health snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostgresHealthReport {
    /// Total connections currently owned by the pool.
    pub connections: u32,
    /// Connections currently idle and immediately available.
    pub idle_connections: u32,
    /// Whether certificate-validated TLS is active.
    pub tls_enabled: bool,
}

/// Reusable pooled PostgreSQL connector.
#[derive(Clone)]
pub struct PostgresPool {
    inner: Arc<PostgresPoolInner>,
}

impl PostgresPool {
    /// Builds a PostgreSQL connection pool from validated config.
    pub async fn connect(config: &PostgresConfig) -> PostgresResult<Self> {
        let postgres_config = configured_client(config)?;
        let inner = match config.transport_security() {
            PostgresTransportSecurity::RequireTls => {
                let manager =
                    PostgresConnectionManager::new(postgres_config, tls_connector(config)?);
                PostgresPoolInner::Tls(build_pool(config, manager).await?)
            }
            PostgresTransportSecurity::AllowPlaintextForDevelopment => {
                let manager = PostgresConnectionManager::new(postgres_config, NoTls);
                PostgresPoolInner::PlaintextDevelopment(build_pool(config, manager).await?)
            }
        };
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Returns a pooled PostgreSQL connection.
    pub async fn get(&self) -> PostgresResult<PostgresPooledConnection<'_>> {
        let inner = match self.inner.as_ref() {
            PostgresPoolInner::Tls(pool) => PostgresPooledConnectionInner::Tls(
                pool.get().await.map_err(|_error| connection_error())?,
            ),
            PostgresPoolInner::PlaintextDevelopment(pool) => {
                PostgresPooledConnectionInner::PlaintextDevelopment(
                    pool.get().await.map_err(|_error| connection_error())?,
                )
            }
        };
        Ok(PostgresPooledConnection { inner })
    }

    /// Runs a lightweight readiness query through the pool.
    pub async fn health_check(&self) -> PostgresResult<()> {
        self.health_report().await.map(|_report| ())
    }

    /// Runs a readiness query and returns bounded operational pool state.
    pub async fn health_report(&self) -> PostgresResult<PostgresHealthReport> {
        let connection = self.get().await?;
        connection
            .query_one("SELECT 1", &[])
            .await
            .map_err(|error| PostgresError::from_query_error(&error))?;
        let (state, tls_enabled) = match self.inner.as_ref() {
            PostgresPoolInner::Tls(pool) => (pool.state(), true),
            PostgresPoolInner::PlaintextDevelopment(pool) => (pool.state(), false),
        };
        Ok(PostgresHealthReport {
            connections: state.connections,
            idle_connections: state.idle_connections,
            tls_enabled,
        })
    }
}

impl std::fmt::Debug for PostgresPool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PostgresPool")
            .field("inner", &"<redacted-postgres-pool>")
            .finish()
    }
}

fn configured_client(config: &PostgresConfig) -> PostgresResult<TokioPostgresConfig> {
    let mut value = config
        .connection_uri()
        .expose_secret()
        .parse::<TokioPostgresConfig>()
        .map_err(|_error| PostgresError::Setup {
            reason: PostgresSetupErrorReason::InvalidConnectionUri,
        })?;
    if let Some(application_name) = config.application_name() {
        value.application_name(application_name);
    }
    value.connect_timeout(config.connection_timeout());
    value.options(config.session_options().as_str());
    value.ssl_mode(match config.transport_security() {
        PostgresTransportSecurity::RequireTls => SslMode::Require,
        PostgresTransportSecurity::AllowPlaintextForDevelopment => SslMode::Disable,
    });
    Ok(value)
}

fn tls_connector(config: &PostgresConfig) -> PostgresResult<MakeRustlsConnect> {
    let mut roots = rustls::RootCertStore::empty();
    match config.tls_trust() {
        PostgresTlsTrust::NativeRoots => add_native_roots(&mut roots)?,
        PostgresTlsTrust::CustomRootCertificate(_) => {
            let path =
                config
                    .tls_trust()
                    .custom_root_certificate()
                    .ok_or(PostgresError::Setup {
                        reason: PostgresSetupErrorReason::TlsTrustUnavailable,
                    })?;
            add_custom_root(&mut roots, path)?;
        }
    }
    if roots.is_empty() {
        return Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustUnavailable,
        });
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_error| PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsConfigurationUnavailable,
        })?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(MakeRustlsConnect::new(tls))
}

fn add_native_roots(roots: &mut rustls::RootCertStore) -> PostgresResult<()> {
    let native = rustls_native_certs::load_native_certs();
    let (accepted, _ignored) = roots.add_parsable_certificates(native.certs);
    if accepted == 0 {
        return Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustUnavailable,
        });
    }
    Ok(())
}

fn add_custom_root(
    roots: &mut rustls::RootCertStore,
    path: &std::path::Path,
) -> PostgresResult<()> {
    let file = std::fs::File::open(path).map_err(|_error| PostgresError::Setup {
        reason: PostgresSetupErrorReason::TlsTrustUnavailable,
    })?;
    let metadata = file.metadata().map_err(|_error| PostgresError::Setup {
        reason: PostgresSetupErrorReason::TlsTrustUnavailable,
    })?;
    if !metadata.is_file() {
        return Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustInvalid,
        });
    }
    if metadata.len() > MAX_TLS_CA_PEM_BYTES {
        return Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustTooLarge,
        });
    }
    // Read through a bounded adapter as well as checking metadata. The file may
    // be replaced or extended between those operations, so metadata alone is
    // not a sufficient allocation boundary for an administrator-provided path.
    let read_limit = MAX_TLS_CA_PEM_BYTES
        .checked_add(1)
        .ok_or(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustTooLarge,
        })?;
    let mut pem = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut pem)
        .map_err(|_error| PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustUnavailable,
        })?;
    let pem_length = u64::try_from(pem.len()).map_err(|_error| PostgresError::Setup {
        reason: PostgresSetupErrorReason::TlsTrustTooLarge,
    })?;
    if pem_length > MAX_TLS_CA_PEM_BYTES {
        return Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustTooLarge,
        });
    }

    let mut accepted = 0_u32;
    for certificate in rustls::pki_types::CertificateDer::pem_slice_iter(&pem) {
        let certificate = certificate.map_err(|_error| PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustInvalid,
        })?;
        roots
            .add(certificate)
            .map_err(|_error| PostgresError::Setup {
                reason: PostgresSetupErrorReason::TlsTrustInvalid,
            })?;
        accepted = accepted.checked_add(1).ok_or(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustTooLarge,
        })?;
        if accepted > MAX_TLS_CA_CERTIFICATES {
            return Err(PostgresError::Setup {
                reason: PostgresSetupErrorReason::TlsTrustTooLarge,
            });
        }
    }
    if accepted == 0 {
        return Err(PostgresError::Setup {
            reason: PostgresSetupErrorReason::TlsTrustInvalid,
        });
    }
    Ok(())
}

async fn build_pool<M>(config: &PostgresConfig, manager: M) -> PostgresResult<bb8::Pool<M>>
where
    M: bb8::ManageConnection,
{
    bb8::Pool::builder()
        .max_size(config.max_pool_size())
        // Eagerly establish at least the configured floor so pool construction
        // is a real startup connectivity gate rather than a lazy allocation.
        .min_idle(config.min_pool_size())
        .connection_timeout(config.connection_timeout())
        .build(manager)
        .await
        .map_err(|_error| PostgresError::Setup {
            reason: PostgresSetupErrorReason::PoolUnavailable,
        })
}

fn connection_error() -> PostgresError {
    PostgresError::Query {
        reason: PostgresQueryErrorReason::ConnectionUnavailable,
    }
}

#[cfg(test)]
#[path = "connector_tests.rs"]
mod tests;
