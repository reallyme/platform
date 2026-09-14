// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FoundationDbTenantName-scoped FoundationDB access.
//!
//! This module owns tenant lifecycle checks and tenant-scoped transactions.
//! Transaction helpers are intentionally centralized so production call-sites can share
//! retry and cancellation assumptions.

use std::{future::Future, pin::Pin, sync::Arc};

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::{
    Database, TransactError, TransactOption, Transaction,
    tenant::{FdbTenant, TenantManagement},
};

use crate::fdb::error::{FdbError, FdbResult, TenantErrorReason};

mod metadata;
mod transact;
use metadata::read_tenant_metadata;
#[cfg(feature = "metrics")]
use transact::record_tenant_open_failure;
use transact::{TenantTransactArcData, TenantTransactFnMutData, tenant_handle_transact};

/// Explicit operator-only tenant lifecycle operations.
#[cfg(feature = "tenant-admin")]
pub mod admin;

const FDB_NOT_COMMITTED_ERROR_CODE: i32 = 1020;
const MIN_C_API_TIMEOUT_MILLIS: i32 = 1;

#[cfg(feature = "metrics")]
const METRIC_FDB_TRANSACTION_ATTEMPTS_TOTAL: &str = "reallyme_fdb_transaction_attempts_total";
#[cfg(feature = "metrics")]
const METRIC_FDB_TRANSACTION_RETRIES_TOTAL: &str = "reallyme_fdb_transaction_retries_total";
#[cfg(feature = "metrics")]
const METRIC_FDB_TRANSACTION_CONFLICTS_TOTAL: &str = "reallyme_fdb_transaction_conflicts_total";
#[cfg(feature = "metrics")]
const METRIC_FDB_TRANSACTION_COMMIT_LATENCY_SECONDS: &str =
    "reallyme_fdb_transaction_commit_latency_seconds";
#[cfg(feature = "metrics")]
const METRIC_FDB_TENANT_OPEN_FAILURES_TOTAL: &str = "reallyme_fdb_tenant_open_failures_total";
#[cfg(feature = "metrics")]
const METRIC_LABEL_OPERATION_CLASS: &str = "operation_class";
#[cfg(feature = "metrics")]
const METRIC_LABEL_TENANT: &str = "tenant";

#[derive(Clone, Copy)]
enum TenantTransactionOperationClass {
    Read,
    Write,
}

impl TenantTransactionOperationClass {
    #[cfg(feature = "metrics")]
    fn as_label(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }
}

/// FoundationDbTenantName-scoped FoundationDB handle.
///
/// Construct via [`crate::FdbContext::open_tenant`]. Transactions opened from this
/// handle are automatically scoped to the underlying FoundationDB tenant.
pub struct TenantHandle {
    tenant: FoundationDbTenantName,
    inner: FdbTenant,
    // Runtime-opened tenants retain the process network until their C handle is
    // destroyed. Admin-only temporary handles instead borrow a caller-owned DB.
    _runtime_owner: Option<crate::fdb::connector::FoundationDbConnector>,
}

impl TenantHandle {
    pub(crate) fn new(tenant: FoundationDbTenantName, inner: FdbTenant) -> Self {
        Self {
            tenant,
            inner,
            _runtime_owner: None,
        }
    }

    /// Returns the logical tenant this handle is scoped to.
    pub fn tenant(&self) -> FoundationDbTenantName {
        self.tenant
    }

    /// Returns the underlying FoundationDB tenant handle.
    ///
    /// This is an explicit and reviewed escape hatch for admin-only flows where
    /// tenant scoping is intentionally bypassed. Keep production call-sites on
    /// [`Self::transact_boxed`] and [`Self::transact_boxed_arc`].
    ///
    /// Audit note: current approved callsites are confined to admin-oriented
    /// workflows and tests that require direct tenant handle access.
    #[allow(dead_code)]
    pub(crate) fn inner(&self) -> &FdbTenant {
        &self.inner
    }

    /// Runs a transactional closure against the tenant handle.
    ///
    /// This wrapper preserves FoundationDB retry semantics for transaction errors,
    /// including maybe-committed handling.
    ///
    /// Invariant notes for cancellation and at-most-once:
    /// - FoundationDB may call the closure repeatedly after `trx.on_error`; any
    ///   mutable state passed in `data` must therefore be designed to survive
    ///   replay and must not assume single-shot execution.
    /// - Callers choose `is_idempotent` via `idempotent_read_option` and
    ///   `mutation_option`; non-idempotent writes must avoid implicitly retrying a
    ///   `maybe_committed` commit.
    /// - Cancellation of the caller future remains propagated into the FoundationDB
    ///   transaction.
    ///
    /// Prefer [`Self::transact_boxed_arc`] for callsites where `D` is expensive
    /// to copy or should be reused read-only across retries.
    pub async fn transact_boxed<'trx, F, D, T, E>(
        &'trx self,
        data: D,
        f: F,
        options: TransactOption,
    ) -> Result<T, E>
    where
        F: for<'a> FnMut(
            &'a Transaction,
            &'a mut D,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
        E: TransactError + Send + 'trx,
        T: Send + 'trx,
        D: Send + 'trx,
        F: Send + 'trx,
    {
        let operation_class = if options.is_idempotent {
            TenantTransactionOperationClass::Read
        } else {
            TenantTransactionOperationClass::Write
        };

        tenant_handle_transact(
            &self.inner,
            TenantTransactFnMutData::new(f, data),
            options,
            operation_class,
            self.tenant,
        )
        .await
    }

    /// Same as [`Self::transact_boxed`] but uses shared immutable closure data.
    ///
    /// This is the preferred shape for operations that only need shared read-only
    /// arguments and run frequently.
    pub async fn transact_boxed_arc<'trx, F, D, T, E>(
        &'trx self,
        data: Arc<D>,
        f: F,
        options: TransactOption,
    ) -> Result<T, E>
    where
        F: for<'a> FnMut(
            &'a Transaction,
            &'a Arc<D>,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
        E: TransactError + Send + 'trx,
        T: Send + 'trx,
        D: Send + Sync + 'trx,
        F: Send + 'trx,
    {
        let operation_class = if options.is_idempotent {
            TenantTransactionOperationClass::Read
        } else {
            TenantTransactionOperationClass::Write
        };

        tenant_handle_transact(
            &self.inner,
            TenantTransactArcData::new(f, data),
            options,
            operation_class,
            self.tenant,
        )
        .await
    }
}

/// Opens a tenant-scoped handle and validates per-tenant metadata.
pub(crate) async fn open_provisioned(
    database: &Database,
    tenant: FoundationDbTenantName,
    runtime_owner: crate::fdb::connector::FoundationDbConnector,
) -> FdbResult<Arc<TenantHandle>> {
    let label = tenant.as_bytes();

    let tenant_handle = match TenantManagement::get_tenant(database, label).await {
        Ok(Some(_info)) => {
            let inner = database.open_tenant(label).map_err(|_| FdbError::Tenant {
                reason: TenantErrorReason::OpenFailed { tenant },
            })?;
            let mut handle = TenantHandle::new(tenant, inner);
            handle._runtime_owner = Some(runtime_owner);
            Arc::new(handle)
        }
        Ok(None) => {
            #[cfg(feature = "metrics")]
            record_tenant_open_failure(tenant);
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::NotProvisioned { tenant },
            });
        }
        Err(_) => {
            #[cfg(feature = "metrics")]
            record_tenant_open_failure(tenant);
            return Err(FdbError::Tenant {
                reason: TenantErrorReason::LookupFailed { tenant },
            });
        }
    };

    let metadata = read_tenant_metadata(&tenant_handle).await?;
    metadata.ensure_compatible(tenant)?;

    Ok(tenant_handle)
}

#[cfg(test)]
#[path = "tenant_tests.rs"]
mod tests;
