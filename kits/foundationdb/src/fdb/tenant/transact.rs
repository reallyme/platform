// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FoundationDB tenant transaction retry execution and instrumentation.

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc, time::Instant};

use foundationdb::{
    DatabaseTransact, TransactError, TransactOption, Transaction, options::TransactionOption,
    tenant::FdbTenant,
};
#[cfg(feature = "metrics")]
use metrics::{counter, histogram};

use super::{
    FDB_NOT_COMMITTED_ERROR_CODE, MIN_C_API_TIMEOUT_MILLIS, TenantTransactionOperationClass,
};
#[cfg(feature = "metrics")]
use super::{
    METRIC_FDB_TENANT_OPEN_FAILURES_TOTAL, METRIC_FDB_TRANSACTION_ATTEMPTS_TOTAL,
    METRIC_FDB_TRANSACTION_COMMIT_LATENCY_SECONDS, METRIC_FDB_TRANSACTION_CONFLICTS_TOTAL,
    METRIC_FDB_TRANSACTION_RETRIES_TOTAL, METRIC_LABEL_OPERATION_CLASS, METRIC_LABEL_TENANT,
};
use crate::fdb::tenant_name::FoundationDbTenantName;

/// A typed closure adapter for FoundationDB's `DatabaseTransact` retry contract.
pub(super) struct TenantTransactFnMutData<'trx, F, D> {
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
    pub(super) const fn new(f: F, d: D) -> Self {
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

pub(super) struct TenantTransactArcData<'trx, F, D> {
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
    pub(super) const fn new(f: F, d: Arc<D>) -> Self {
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
pub(super) fn record_tenant_open_failure(tenant: FoundationDbTenantName) {
    counter!(
        METRIC_FDB_TENANT_OPEN_FAILURES_TOTAL,
        "tenant" => tenant.to_string(),
        "result" => "failed"
    )
    .increment(1);
}

pub(super) const fn retry_permitted(
    error_is_retryable: bool,
    error_is_maybe_committed: bool,
    operation_is_idempotent: bool,
    budget_allows_retry: bool,
) -> bool {
    error_is_retryable
        && (operation_is_idempotent || !error_is_maybe_committed)
        && budget_allows_retry
}

pub(super) fn c_api_timeout_millis(duration: std::time::Duration) -> i32 {
    i32::try_from(duration.as_millis())
        .unwrap_or(i32::MAX)
        .max(MIN_C_API_TIMEOUT_MILLIS)
}

pub(super) fn c_api_retry_limit(total_attempts: u32) -> i32 {
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

pub(super) async fn tenant_handle_transact<'trx, T, E>(
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
