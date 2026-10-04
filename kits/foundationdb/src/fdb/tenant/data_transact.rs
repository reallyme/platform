// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Retry adapters for application transactions with restricted key access.

use std::{future::Future, marker::PhantomData, pin::Pin, sync::Arc};

use foundationdb::{DatabaseTransact, TransactError, Transaction};

use super::data::TenantDataTransaction;

/// Carries mutable closure state through FoundationDB's retry contract.
pub(super) struct TenantDataFnMutAdapter<'trx, F, D> {
    operation: F,
    data: D,
    _marker: PhantomData<&'trx ()>,
}

impl<'trx, F, D> TenantDataFnMutAdapter<'trx, F, D> {
    pub(super) const fn new(operation: F, data: D) -> Self {
        Self {
            operation,
            data,
            _marker: PhantomData,
        }
    }
}

impl<'trx, F, D, T, E> DatabaseTransact for TenantDataFnMutAdapter<'trx, F, D>
where
    F: for<'a> FnMut(
            &'a TenantDataTransaction<'a>,
            &'a mut D,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>
        + Send
        + 'trx,
    E: TransactError + Send + 'trx,
    T: Send + 'trx,
    D: Send + 'trx,
{
    type Item = T;
    type Error = E;
    type Future = Pin<
        Box<
            dyn Future<Output = (Self, Transaction, Result<Self::Item, Self::Error>)> + Send + 'trx,
        >,
    >;

    fn transact(self, transaction: Transaction) -> Self::Future {
        let mut operation = self.operation;
        let mut data = self.data;
        Box::pin(async move {
            let view = TenantDataTransaction::new(&transaction);
            let result = operation(&view, &mut data).await;
            (
                Self {
                    operation,
                    data,
                    _marker: PhantomData,
                },
                transaction,
                result,
            )
        })
    }
}

/// Carries immutable shared data through FoundationDB's retry contract.
pub(super) struct TenantDataArcAdapter<'trx, F, D> {
    operation: F,
    data: Arc<D>,
    _marker: PhantomData<&'trx ()>,
}

impl<'trx, F, D> TenantDataArcAdapter<'trx, F, D> {
    pub(super) const fn new(operation: F, data: Arc<D>) -> Self {
        Self {
            operation,
            data,
            _marker: PhantomData,
        }
    }
}

impl<'trx, F, D, T, E> DatabaseTransact for TenantDataArcAdapter<'trx, F, D>
where
    F: for<'a> FnMut(
            &'a TenantDataTransaction<'a>,
            &'a Arc<D>,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>
        + Send
        + 'trx,
    E: TransactError + Send + 'trx,
    T: Send + 'trx,
    D: Send + Sync + 'trx,
{
    type Item = T;
    type Error = E;
    type Future = Pin<
        Box<
            dyn Future<Output = (Self, Transaction, Result<Self::Item, Self::Error>)> + Send + 'trx,
        >,
    >;

    fn transact(self, transaction: Transaction) -> Self::Future {
        let mut operation = self.operation;
        let data = self.data;
        Box::pin(async move {
            let view = TenantDataTransaction::new(&transaction);
            let result = operation(&view, &data).await;
            (
                Self {
                    operation,
                    data,
                    _marker: PhantomData,
                },
                transaction,
                result,
            )
        })
    }
}
