// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FoundationDbTenantName-scoped FoundationDB access.
//!
//! This module owns tenant lifecycle checks and tenant-scoped transactions.
//! Transaction helpers are intentionally centralized so production call-sites can share
//! retry and cancellation assumptions.

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc, time::Instant};

use crate::fdb::tenant_name::FoundationDbTenantName;
use foundationdb::{
    Database, DatabaseTransact, TransactError, TransactOption, Transaction,
    options::TransactionOption,
    tenant::{FdbTenant, TenantManagement},
};

use crate::fdb::error::{FdbError, FdbResult, TenantErrorReason};

mod metadata;
use metadata::read_tenant_metadata;

/// Explicit operator-only tenant lifecycle operations.
#[cfg(feature = "tenant-admin")]
pub mod admin;

#[cfg(feature = "metrics")]
use metrics::{counter, histogram};

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

/// A typed closure adapter for FoundationDB's `DatabaseTransact` retry contract.
struct TenantTransactFnMutData<'trx, F, D> {
    f: F,
    d: D,
    _marker: PhantomData<&'trx ()>,
}

impl<'trx, F, D, T, E> TenantTransactFnMutData<'trx, F, D>
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
    const fn new(f: F, d: D) -> Self {
        Self {
            f,
            d,
            _marker: PhantomData,
        }
    }
}

impl<'trx, F, D, T, E> DatabaseTransact for TenantTransactFnMutData<'trx, F, D>
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
    type Item = T;
    type Error = E;
    type Future = Pin<
        Box<
            dyn Future<Output = (Self, Transaction, Result<Self::Item, Self::Error>)> + Send + 'trx,
        >,
    >;

    fn transact(self, trx: Transaction) -> Self::Future {
        let mut f = self.f;
        let mut d = self.d;

        Box::pin(async move {
            let result = f(&trx, &mut d).await;
            (
                TenantTransactFnMutData {
                    f,
                    d,
                    _marker: PhantomData,
                },
                trx,
                result,
            )
        })
    }
}

struct TenantTransactArcData<'trx, F, D> {
    f: F,
    d: Arc<D>,
    _marker: PhantomData<&'trx ()>,
}

impl<'trx, F, D, T, E> TenantTransactArcData<'trx, F, D>
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
    const fn new(f: F, d: Arc<D>) -> Self {
        Self {
            f,
            d,
            _marker: PhantomData,
        }
    }
}

impl<'trx, F, D, T, E> DatabaseTransact for TenantTransactArcData<'trx, F, D>
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
    type Item = T;
    type Error = E;
    type Future = Pin<
        Box<
            dyn Future<Output = (Self, Transaction, Result<Self::Item, Self::Error>)> + Send + 'trx,
        >,
    >;

    fn transact(self, trx: Transaction) -> Self::Future {
        let mut f = self.f;
        let d = self.d;

        Box::pin(async move {
            let result = f(&trx, &d).await;
            (
                TenantTransactArcData {
                    f,
                    d,
                    _marker: PhantomData,
                },
                trx,
                result,
            )
        })
    }
}

#[cfg(feature = "metrics")]
fn record_transaction_metrics(
    tenant: FoundationDbTenantName,
    operation_class: &'static str,
    attempts: u32,
    retries: u32,
) {
    counter!(
        METRIC_FDB_TRANSACTION_ATTEMPTS_TOTAL,
        METRIC_LABEL_OPERATION_CLASS => operation_class,
        METRIC_LABEL_TENANT => tenant.to_string(),
    )
    .increment(u64::from(attempts));
    if retries > 0 {
        counter!(
            METRIC_FDB_TRANSACTION_RETRIES_TOTAL,
            METRIC_LABEL_OPERATION_CLASS => operation_class,
            METRIC_LABEL_TENANT => tenant.to_string(),
        )
        .increment(u64::from(retries));
    }
}

#[cfg(feature = "metrics")]
fn record_transaction_conflicts(
    tenant: FoundationDbTenantName,
    operation_class: &'static str,
    conflicts: u32,
) {
    if conflicts > 0 {
        counter!(
            METRIC_FDB_TRANSACTION_CONFLICTS_TOTAL,
            METRIC_LABEL_OPERATION_CLASS => operation_class,
            METRIC_LABEL_TENANT => tenant.to_string(),
        )
        .increment(u64::from(conflicts));
    }
}

#[cfg(feature = "metrics")]
fn record_commit_latency(
    tenant: FoundationDbTenantName,
    operation_class: &'static str,
    duration: std::time::Duration,
) {
    histogram!(
        METRIC_FDB_TRANSACTION_COMMIT_LATENCY_SECONDS,
        METRIC_LABEL_OPERATION_CLASS => operation_class,
        METRIC_LABEL_TENANT => tenant.to_string()
    )
    .record(duration.as_secs_f64());
}

#[cfg(feature = "metrics")]
fn record_tenant_open_failure(tenant: FoundationDbTenantName) {
    counter!(
        METRIC_FDB_TENANT_OPEN_FAILURES_TOTAL,
        "tenant" => tenant.to_string(),
        "result" => "failed"
    )
    .increment(1);
}

const fn retry_permitted(
    error_is_retryable: bool,
    error_is_maybe_committed: bool,
    operation_is_idempotent: bool,
    budget_allows_retry: bool,
) -> bool {
    error_is_retryable
        && (operation_is_idempotent || !error_is_maybe_committed)
        && budget_allows_retry
}

fn c_api_timeout_millis(duration: std::time::Duration) -> i32 {
    i32::try_from(duration.as_millis())
        .unwrap_or(i32::MAX)
        .max(MIN_C_API_TIMEOUT_MILLIS)
}

fn c_api_retry_limit(total_attempts: u32) -> i32 {
    i32::try_from(total_attempts.saturating_sub(1)).unwrap_or(i32::MAX)
}

fn apply_c_api_limits(
    transaction: &Transaction,
    options: TransactOption,
) -> foundationdb::FdbResult<()> {
    if let Some(duration) = options.time_out {
        // A zero C API timeout disables cancellation. Clamp zero to one
        // millisecond so even manually constructed TransactOption values fail
        // closed, while oversized durations remain bounded by the C API limit.
        transaction.set_option(TransactionOption::Timeout(c_api_timeout_millis(duration)))?;
    }

    if let Some(total_attempts) = options.retry_limit {
        transaction.set_option(TransactionOption::RetryLimit(c_api_retry_limit(
            total_attempts,
        )))?;
    }

    Ok(())
}

async fn tenant_handle_transact<'trx, T, E>(
    tenant: &FdbTenant,
    mut f: impl DatabaseTransact<Item = T, Error = E> + Send + 'trx,
    options: TransactOption,
    _operation_class: TenantTransactionOperationClass,
    _tenant_label: FoundationDbTenantName,
) -> Result<T, E>
where
    T: Send + 'trx,
    E: TransactError + Send + 'trx,
{
    let is_idempotent = options.is_idempotent;
    #[cfg(feature = "metrics")]
    let operation_class = _operation_class.as_label();
    #[cfg(feature = "metrics")]
    let tenant_label = _tenant_label;
    let mut retries: u32 = 0;
    let mut attempts: u32 = 0;
    let mut conflicts: u32 = 0;
    let started_at = Instant::now();
    let time_out = options
        .time_out
        .map(|duration| started_at.checked_add(duration).unwrap_or(started_at));
    let retry_limit = options.retry_limit;
    let mut trx = tenant.create_trx()?;
    apply_c_api_limits(&trx, options).map_err(E::from)?;
    let can_retry = |completed_attempts: u32| {
        retry_limit
            .map(|limit| completed_attempts < limit)
            .unwrap_or(true)
            && time_out
                .map(|deadline| Instant::now() < deadline)
                .unwrap_or(true)
    };

    #[cfg(feature = "metrics")]
    let started = Instant::now();

    loop {
        attempts = attempts.saturating_add(1);
        let r = f.transact(trx).await;
        f = r.0;
        trx = r.1;

        match r.2 {
            Ok(item) => match trx.commit().await {
                Ok(_) => {
                    #[cfg(feature = "metrics")]
                    let elapsed = started.elapsed();
                    #[cfg(feature = "metrics")]
                    record_transaction_metrics(tenant_label, operation_class, attempts, retries);
                    #[cfg(feature = "metrics")]
                    record_transaction_conflicts(tenant_label, operation_class, conflicts);
                    #[cfg(feature = "metrics")]
                    record_commit_latency(tenant_label, operation_class, elapsed);
                    return Ok(item);
                }
                Err(error) => {
                    let should_retry = retry_permitted(
                        error.is_retryable(),
                        error.is_maybe_committed(),
                        is_idempotent,
                        can_retry(attempts),
                    );
                    if error.code() == FDB_NOT_COMMITTED_ERROR_CODE {
                        conflicts = conflicts.saturating_add(1);
                    }
                    if should_retry {
                        match error.on_error().await {
                            Ok(next_transaction) => {
                                retries = retries.saturating_add(1);
                                trx = next_transaction;
                            }
                            Err(on_error) => {
                                #[cfg(feature = "metrics")]
                                record_transaction_metrics(
                                    tenant_label,
                                    operation_class,
                                    attempts,
                                    retries,
                                );
                                #[cfg(feature = "metrics")]
                                record_transaction_conflicts(
                                    tenant_label,
                                    operation_class,
                                    conflicts,
                                );
                                return Err(E::from(on_error));
                            }
                        }
                    } else {
                        #[cfg(feature = "metrics")]
                        record_transaction_metrics(
                            tenant_label,
                            operation_class,
                            attempts,
                            retries,
                        );
                        #[cfg(feature = "metrics")]
                        record_transaction_conflicts(tenant_label, operation_class, conflicts);
                        return Err(E::from(error.into()));
                    }
                }
            },
            Err(user_error) => match user_error.try_into_fdb_error() {
                Ok(fdb_error) => {
                    let should_retry = retry_permitted(
                        fdb_error.is_retryable(),
                        fdb_error.is_maybe_committed(),
                        is_idempotent,
                        can_retry(attempts),
                    );
                    if fdb_error.code() == FDB_NOT_COMMITTED_ERROR_CODE {
                        conflicts = conflicts.saturating_add(1);
                    }
                    if should_retry {
                        match trx.on_error(fdb_error).await {
                            Ok(next_transaction) => {
                                retries = retries.saturating_add(1);
                                trx = next_transaction;
                            }
                            Err(on_error) => {
                                #[cfg(feature = "metrics")]
                                record_transaction_metrics(
                                    tenant_label,
                                    operation_class,
                                    attempts,
                                    retries,
                                );
                                #[cfg(feature = "metrics")]
                                record_transaction_conflicts(
                                    tenant_label,
                                    operation_class,
                                    conflicts,
                                );
                                return Err(E::from(on_error));
                            }
                        }
                    } else {
                        #[cfg(feature = "metrics")]
                        record_transaction_metrics(
                            tenant_label,
                            operation_class,
                            attempts,
                            retries,
                        );
                        #[cfg(feature = "metrics")]
                        record_transaction_conflicts(tenant_label, operation_class, conflicts);
                        return Err(E::from(fdb_error));
                    }
                }
                Err(err) => {
                    #[cfg(feature = "metrics")]
                    record_transaction_metrics(tenant_label, operation_class, attempts, retries);
                    #[cfg(feature = "metrics")]
                    record_transaction_conflicts(tenant_label, operation_class, conflicts);
                    return Err(err);
                }
            },
        }
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
mod tests {
    use std::time::Duration;

    use super::{c_api_retry_limit, c_api_timeout_millis, retry_permitted};

    #[test]
    fn retry_requires_retryable_error_and_remaining_budget() {
        assert!(retry_permitted(true, false, false, true));
        assert!(!retry_permitted(false, false, true, true));
        assert!(!retry_permitted(true, false, true, false));
    }

    #[test]
    fn maybe_committed_is_retried_only_for_idempotent_operations() {
        assert!(retry_permitted(true, true, true, true));
        assert!(!retry_permitted(true, true, false, true));
    }

    #[test]
    fn c_api_limits_fail_closed_for_unvalidated_options() {
        assert_eq!(c_api_timeout_millis(Duration::ZERO), 1);
        assert_eq!(c_api_timeout_millis(Duration::from_millis(500)), 500);
        assert_eq!(c_api_timeout_millis(Duration::MAX), i32::MAX);
        assert_eq!(c_api_retry_limit(1), 0);
        assert_eq!(c_api_retry_limit(u32::MAX), i32::MAX);
    }
}
