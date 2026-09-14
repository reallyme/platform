// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Process-scoped FoundationDB connector and readiness checks.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "metrics")]
use std::time::Instant;

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::{Database, api::NetworkAutoStop, options::TransactionOption};

use crate::fdb::{
    config::FdbConfig,
    error::{FdbError, FdbHealthErrorReason, FdbResult, FdbSetupErrorReason},
    tenant::{TenantHandle, open_provisioned},
};

#[cfg(feature = "metrics")]
use metrics::{counter, histogram};

static CLIENT_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "metrics")]
const METRIC_HEALTH_CHECKS_TOTAL: &str = "reallyme_fdb_health_checks_total";
#[cfg(feature = "metrics")]
const METRIC_HEALTH_CHECK_LATENCY_SECONDS: &str = "reallyme_fdb_health_check_latency_seconds";

/// Low-cardinality FoundationDB health snapshot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FdbHealthReport {
    /// Runtime API behavior selected when the process initialized FoundationDB.
    pub api_version: i32,
    /// Cluster read version observed by the readiness transaction.
    pub read_version: i64,
    /// Client network-thread saturation, when supported by the runtime API.
    pub main_thread_busyness: Option<f64>,
}

struct FdbConnectorInner {
    // This guard must outlive every database and tenant handle. Keeping it in
    // the same Arc as the database makes cloned connectors lifecycle-safe.
    database: Database,
    // Fields drop in declaration order: destroy the database before stopping C I/O.
    _network: NetworkAutoStop,
    api_version: i32,
    health_check_timeout: std::time::Duration,
}

/// Cloneable, process-scoped FoundationDB connector.
///
/// FoundationDB permits one client API/network initialization per process.
/// `connect` enforces that invariant without allowing the upstream binding's
/// duplicate-initialization panic to cross this audit boundary. Clone this
/// value to share the initialized connector; do not call `connect` twice.
#[derive(Clone)]
pub struct FoundationDbConnector {
    inner: Arc<FdbConnectorInner>,
}

/// Backwards-compatible name for [`FoundationDbConnector`].
pub type FdbContext = FoundationDbConnector;

impl FoundationDbConnector {
    /// Initializes the singleton FoundationDB client and opens a database handle.
    ///
    /// Database creation in the FoundationDB C API is lazy. Production startup
    /// should use [`Self::connect_and_check`] or call [`Self::health_check`]
    /// before accepting traffic.
    pub fn connect(config: &FdbConfig) -> FdbResult<Self> {
        CLIENT_INITIALIZED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| FdbError::Setup {
                reason: FdbSetupErrorReason::ClientAlreadyInitialized,
            })?;

        let network_builder = catch_unwind(AssertUnwindSafe(|| {
            foundationdb::api::FdbApiBuilder::default()
                .set_runtime_version(config.fdb_api_version())
                .build()
        }))
        .map_err(|_| FdbError::Setup {
            reason: FdbSetupErrorReason::ClientInitializationPanicked,
        })?
        .map_err(|_| FdbError::Setup {
            reason: FdbSetupErrorReason::ApiVersionUnsupported,
        })?;

        // FoundationDB marks boot unsafe because the returned guard must be
        // dropped before normal process exit. FdbConnectorInner owns that guard
        // and Arc ownership keeps it alive until all connector clones are gone.
        #[allow(unsafe_code)]
        let network = catch_unwind(AssertUnwindSafe(|| unsafe { network_builder.boot() }))
            .map_err(|_| FdbError::Setup {
                reason: FdbSetupErrorReason::ClientInitializationPanicked,
            })?
            .map_err(|_| FdbError::Setup {
                reason: FdbSetupErrorReason::NetworkBootFailed,
            })?;

        let database = match config.fdb_cluster_file() {
            Some(cluster_file) => Database::from_path(cluster_file),
            None => Database::default(),
        }
        .map_err(|_| FdbError::Setup {
            reason: FdbSetupErrorReason::DatabaseOpenFailed,
        })?;

        Ok(Self {
            inner: Arc::new(FdbConnectorInner {
                _network: network,
                database,
                api_version: config.fdb_api_version(),
                health_check_timeout: config.health_check_timeout(),
            }),
        })
    }

    /// Initializes the connector and proves client and cluster reachability.
    pub async fn connect_and_check(config: &FdbConfig) -> FdbResult<Self> {
        let connector = Self::connect(config)?;
        connector.health_check().await?;
        Ok(connector)
    }

    /// Runs a bounded, read-only readiness probe against the cluster.
    pub async fn health_check(&self) -> FdbResult<()> {
        self.health_report().await.map(|_report| ())
    }

    /// Runs a bounded readiness probe and returns operational client state.
    ///
    /// Unlike `Database::perform_no_op`, this also obtains a read version, so a
    /// successful result proves both local network-thread liveness and actual
    /// cluster reachability without reading application keys.
    pub async fn health_report(&self) -> FdbResult<FdbHealthReport> {
        #[cfg(feature = "metrics")]
        let started = Instant::now();
        let probe = tokio::time::timeout(self.inner.health_check_timeout, self.probe()).await;
        let result = match probe {
            Ok(result) => result,
            Err(_) => Err(FdbError::Health {
                reason: FdbHealthErrorReason::DeadlineExceeded,
            }),
        };

        #[cfg(feature = "metrics")]
        record_health_metrics(&result, started.elapsed());

        result
    }

    async fn probe(&self) -> FdbResult<FdbHealthReport> {
        self.inner
            .database
            .perform_no_op()
            .await
            .map_err(|_| FdbError::Health {
                reason: FdbHealthErrorReason::NetworkUnavailable,
            })?;
        let transaction = self
            .inner
            .database
            .create_trx()
            .map_err(|_| FdbError::Health {
                reason: FdbHealthErrorReason::NetworkUnavailable,
            })?;
        // FoundationDbTenantName-required clusters reject ordinary transactions against the
        // root database. Readiness operates before any app tenant is selected,
        // so permit this no-data transaction to access the system namespace;
        // obtaining its read version remains a cluster round trip without
        // reading or mutating system keys.
        transaction
            .set_option(TransactionOption::AccessSystemKeys)
            .map_err(|_| FdbError::Health {
                reason: FdbHealthErrorReason::NetworkUnavailable,
            })?;
        let read_version = transaction
            .get_read_version()
            .await
            .map_err(|_| FdbError::Health {
                reason: FdbHealthErrorReason::ClusterUnavailable,
            })?;
        let main_thread_busyness = if self.inner.api_version >= 710 {
            self.inner.database.get_main_thread_busyness().await.ok()
        } else {
            None
        };

        Ok(FdbHealthReport {
            api_version: self.inner.api_version,
            read_version,
            main_thread_busyness,
        })
    }

    /// Opens a provisioned tenant and validates its metadata schema.
    ///
    /// This fails closed; tenant creation remains an explicit operator action.
    pub async fn open_tenant(
        &self,
        tenant: FoundationDbTenantName,
    ) -> FdbResult<Arc<TenantHandle>> {
        open_provisioned(&self.inner.database, tenant, self.clone()).await
    }

    /// Returns the database handle to crate-internal infrastructure helpers.
    #[allow(dead_code)]
    pub(crate) fn database(&self) -> &Database {
        &self.inner.database
    }

    /// Returns the database handle for explicit operator-only administration.
    #[cfg(feature = "tenant-admin")]
    pub fn database_for_admin(&self) -> &Database {
        &self.inner.database
    }
}

impl std::fmt::Debug for FoundationDbConnector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FoundationDbConnector")
            .field("api_version", &self.inner.api_version)
            .field("health_check_timeout", &self.inner.health_check_timeout)
            .field("database", &"<redacted-foundationdb-handle>")
            .finish()
    }
}

#[cfg(feature = "metrics")]
fn record_health_metrics(result: &FdbResult<FdbHealthReport>, duration: std::time::Duration) {
    let result_label = if result.is_ok() { "ready" } else { "failed" };
    counter!(METRIC_HEALTH_CHECKS_TOTAL, "result" => result_label).increment(1);
    histogram!(METRIC_HEALTH_CHECK_LATENCY_SECONDS).record(duration.as_secs_f64());
}
