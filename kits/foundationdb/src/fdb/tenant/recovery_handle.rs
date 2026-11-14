// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FoundationDB C handles scoped to operator recovery.

// The Rust binding does not expose the resolved ID of its owned tenant handle.
// Keep pointer ownership and all direct C calls within this module.
#![allow(unsafe_code)]

use std::ffi::CString;
use std::ptr::{self, NonNull};

use foundationdb::future::{FdbFuture, FdbFutureHandle, FdbValues};
use foundationdb_sys as sys;

use crate::fdb::error::{FdbError, FdbResult, FdbSetupErrorReason, TenantErrorReason};
use crate::fdb::tenant_name::FoundationDbTenantName;

pub(in crate::fdb) struct RecoveryDatabase(NonNull<sys::FDBDatabase>);

// SAFETY: The C database API is thread-safe and this wrapper owns the sole
// destroy obligation. The connector retains it until its network shuts down.
unsafe impl Send for RecoveryDatabase {}
// SAFETY: Shared methods open independent tenant handles through the C API.
unsafe impl Sync for RecoveryDatabase {}

impl RecoveryDatabase {
    pub(in crate::fdb) fn open(cluster_file: Option<&str>) -> FdbResult<Self> {
        let failed = FdbError::Setup {
            reason: FdbSetupErrorReason::DatabaseOpenFailed,
        };
        let cluster_file = cluster_file
            .map(CString::new)
            .transpose()
            .map_err(|_| failed)?;
        let path = cluster_file
            .as_ref()
            .map_or(ptr::null(), |path| path.as_ptr());
        let mut database = ptr::null_mut();
        // SAFETY: The path is NUL-terminated for this call, or null for the
        // default configuration. The out pointer is writable.
        if unsafe { sys::fdb_create_database(path, &mut database) } != 0 {
            return Err(failed);
        }
        NonNull::new(database).map(Self).ok_or(failed)
    }

    pub(super) fn open_tenant(
        &self,
        name: FoundationDbTenantName,
    ) -> FdbResult<RecoveryTenant<'_>> {
        let bytes = name.as_bytes();
        let len = i32::try_from(bytes.len()).map_err(|_| recovery_failed(name))?;
        let mut tenant = ptr::null_mut();
        // SAFETY: The database and name bytes are valid for the call. On
        // success the caller owns the newly returned tenant.
        if unsafe {
            sys::fdb_database_open_tenant(self.0.as_ptr(), bytes.as_ptr(), len, &mut tenant)
        } != 0
        {
            return Err(recovery_failed(name));
        }
        Ok(RecoveryTenant {
            handle: NonNull::new(tenant).ok_or_else(|| recovery_failed(name))?,
            name,
            _database: self,
        })
    }
}

impl Drop for RecoveryDatabase {
    fn drop(&mut self) {
        // SAFETY: This owner destroys its live database exactly once.
        unsafe { sys::fdb_database_destroy(self.0.as_ptr()) };
    }
}

pub(super) struct RecoveryTenant<'db> {
    handle: NonNull<sys::FDBTenant>,
    name: FoundationDbTenantName,
    _database: &'db RecoveryDatabase,
}

// SAFETY: The C tenant API permits calls from any thread. Ownership and the
// database borrow move together and keep the network live.
unsafe impl Send for RecoveryTenant<'_> {}
// SAFETY: The handle's C API is thread-safe and destruction requires ownership.
unsafe impl Sync for RecoveryTenant<'_> {}

impl<'db> RecoveryTenant<'db> {
    pub(super) async fn verify(self, expected_id: i64) -> FdbResult<VerifiedRecoveryTenant<'db>> {
        // SAFETY: The owned handle is live and is retained for the entire wait.
        let actual_id = recovery_future::<i64>(
            unsafe { sys::fdb_tenant_get_id(self.handle.as_ptr()) },
            self.name,
        )?
        .await
        .map_err(|_| recovery_failed(self.name))?;
        if actual_id != expected_id {
            return Err(recovery_failed(self.name));
        }
        Ok(VerifiedRecoveryTenant(self))
    }
}

impl Drop for RecoveryTenant<'_> {
    fn drop(&mut self) {
        // SAFETY: This owner destroys its live tenant handle exactly once.
        unsafe { sys::fdb_tenant_destroy(self.handle.as_ptr()) };
    }
}

pub(super) struct VerifiedRecoveryTenant<'db>(RecoveryTenant<'db>);

impl VerifiedRecoveryTenant<'_> {
    pub(super) const fn name(&self) -> FoundationDbTenantName {
        self.0.name
    }

    pub(super) fn create_transaction(
        &self,
        timeout_millis: i64,
    ) -> FdbResult<RecoveryTransaction<'_>> {
        let mut transaction = ptr::null_mut();
        // SAFETY: The verified tenant is live and the out pointer is writable.
        // The C client uses this tenant object's resolved ID for transactions.
        if unsafe { sys::fdb_tenant_create_transaction(self.0.handle.as_ptr(), &mut transaction) }
            != 0
        {
            return Err(recovery_failed(self.0.name));
        }
        let transaction = RecoveryTransaction {
            handle: NonNull::new(transaction).ok_or_else(|| recovery_failed(self.0.name))?,
            name: self.0.name,
            _tenant: &self.0,
        };
        transaction.set_timeout(timeout_millis)?;
        Ok(transaction)
    }
}

pub(super) struct RecoveryTransaction<'tenant> {
    handle: NonNull<sys::FDBTransaction>,
    name: FoundationDbTenantName,
    _tenant: &'tenant RecoveryTenant<'tenant>,
}

// SAFETY: The C transaction can move across threads. This wrapper requires
// exclusive access for each operation and retains the tenant and its database.
unsafe impl Send for RecoveryTransaction<'_> {}

impl RecoveryTransaction<'_> {
    fn set_timeout(&self, timeout_millis: i64) -> FdbResult<()> {
        let value = timeout_millis.to_le_bytes();
        let len = i32::try_from(value.len()).map_err(|_| recovery_failed(self.name))?;
        // SAFETY: The transaction is live and the option payload is the C
        // client's required little-endian integer.
        if unsafe {
            sys::fdb_transaction_set_option(
                self.handle.as_ptr(),
                sys::FDBTransactionOption_FDB_TR_OPTION_TIMEOUT,
                value.as_ptr(),
                len,
            )
        } != 0
        {
            return Err(recovery_failed(self.name));
        }
        Ok(())
    }

    pub(super) async fn range_is_empty(&mut self, begin: &[u8], end: &[u8]) -> FdbResult<bool> {
        let begin_len = i32::try_from(begin.len()).map_err(|_| recovery_failed(self.name))?;
        let end_len = i32::try_from(end.len()).map_err(|_| recovery_failed(self.name))?;
        // First-greater-or-equal selectors give [begin, end). Reading one
        // entry suffices, and the non-snapshot read protects the empty range.
        // SAFETY: Both slices and the transaction are live for the C call.
        let future = unsafe {
            sys::fdb_transaction_get_range(
                self.handle.as_ptr(),
                begin.as_ptr(),
                begin_len,
                0,
                1,
                end.as_ptr(),
                end_len,
                0,
                1,
                1,
                0,
                sys::FDBStreamingMode_FDB_STREAMING_MODE_EXACT,
                1,
                0,
                0,
            )
        };
        let values = recovery_future::<FdbValues>(future, self.name)?
            .await
            .map_err(|_| recovery_failed(self.name))?;
        Ok(values.is_empty() && !values.more())
    }

    pub(super) fn set(&mut self, key: &[u8], value: &[u8]) -> FdbResult<()> {
        let key_len = i32::try_from(key.len()).map_err(|_| recovery_failed(self.name))?;
        let value_len = i32::try_from(value.len()).map_err(|_| recovery_failed(self.name))?;
        // SAFETY: The C API copies both slices during the call and the
        // transaction is live. Deferred write errors surface on commit.
        unsafe {
            sys::fdb_transaction_set(
                self.handle.as_ptr(),
                key.as_ptr(),
                key_len,
                value.as_ptr(),
                value_len,
            );
        }
        Ok(())
    }

    pub(super) async fn commit(self) -> FdbResult<()> {
        // SAFETY: The owned transaction remains live for the entire wait.
        let future = unsafe { sys::fdb_transaction_commit(self.handle.as_ptr()) };
        let result = recovery_future::<()>(future, self.name)?
            .await
            .map_err(|_| recovery_failed(self.name));
        drop(self);
        result
    }
}

impl Drop for RecoveryTransaction<'_> {
    fn drop(&mut self) {
        // SAFETY: This owner destroys its live transaction exactly once.
        unsafe { sys::fdb_transaction_destroy(self.handle.as_ptr()) };
    }
}

fn recovery_future<T>(
    future: *mut sys::FDBFuture,
    name: FoundationDbTenantName,
) -> FdbResult<FdbFuture<T>>
where
    T: TryFrom<FdbFutureHandle, Error = foundationdb::FdbError> + Unpin,
{
    let future = NonNull::new(future).ok_or_else(|| recovery_failed(name))?;
    Ok(FdbFuture::new(future.as_ptr()))
}

fn recovery_failed(tenant: FoundationDbTenantName) -> FdbError {
    FdbError::Tenant {
        reason: TenantErrorReason::AdministrationFailed { tenant },
    }
}
