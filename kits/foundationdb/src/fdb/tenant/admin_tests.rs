// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::fdb::tenant_name::FoundationDbTenantName;

use super::metadata_keys;
use super::{
    TenantHandle, clear_metadata_if_empty, delete_tenant, ensure_tenant, finish_delete_after_clear,
    read_tenant_metadata, recover_interrupted_delete, repair_tenant_metadata, restore_metadata,
};
use crate::fdb::config::FdbConfig;
use crate::fdb::connector::FoundationDbConnector;
use crate::fdb::error::{FdbError, TenantErrorReason};
use crate::fdb::tenant::TenantDataKey;
use crate::fdb::transaction::{TenantTransactionPolicy, WriteTxnPolicy};
use foundationdb::tenant::TenantManagement;

#[test]
fn tenant_delete_targets_only_the_kits_two_metadata_keys() {
    let tenant = FoundationDbTenantName::new("example").expect("valid tenant");
    let [schema, created_at] = metadata_keys(tenant).expect("metadata keys");
    assert_eq!(schema.as_ref(), b"__meta/v1/schema_version");
    assert_eq!(created_at.as_ref(), b"__meta/v1/created_at");
}

#[tokio::test]
#[ignore = "requires a live throwaway FoundationDB cluster"]
async fn tenant_delete_recovery_live_cases() {
    let config = FdbConfig::from_env().expect("live FDB configuration");
    // SAFETY: FoundationDB's process-global client is initialized once, and
    // the connector is dropped before this test process exits.
    #[allow(unsafe_code)]
    let connector = unsafe { FoundationDbConnector::connect(&config) }.expect("live FDB connector");
    failed_second_delete_phase_restores_metadata_after_concurrent_write(&connector).await;
    interrupted_delete_recovery_restores_metadata_even_with_application_data(&connector).await;
    restore_write_conflicts_with_in_flight_clear(&connector).await;
    interrupted_creation_requires_explicit_empty_tenant_repair(&connector).await;
}

async fn interrupted_creation_requires_explicit_empty_tenant_repair(
    connector: &FoundationDbConnector,
) {
    let tenant = FoundationDbTenantName::new("kit-create-repair-test").expect("fixed tenant name");
    TenantManagement::create_tenant(connector.database(), tenant.as_bytes())
        .await
        .expect("create tenant before simulated interruption");
    assert!(
        ensure_tenant(connector, tenant).await.is_err(),
        "normal startup must not silently adopt a tenant without kit metadata"
    );
    let inner = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("open interrupted tenant fixture");
    let transaction = inner.create_trx().expect("create foreign-data transaction");
    transaction.set(b"application/data", b"value");
    transaction
        .commit()
        .await
        .expect("write foreign application data");
    assert!(matches!(
        repair_tenant_metadata(connector, tenant).await,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { .. }
        })
    ));
    assert!(
        ensure_tenant(connector, tenant).await.is_err(),
        "refused repair must leave the tenant fail closed"
    );
    let transaction = inner
        .create_trx()
        .expect("create fixture cleanup transaction");
    transaction.clear(b"application/data");
    transaction
        .commit()
        .await
        .expect("remove foreign application data");
    repair_tenant_metadata(connector, tenant)
        .await
        .expect("operator repair of an empty tenant");
    ensure_tenant(connector, tenant)
        .await
        .expect("repaired tenant is compatible");
    delete_tenant(connector, tenant)
        .await
        .expect("remove repair fixture");
}

async fn restore_write_conflicts_with_in_flight_clear(connector: &FoundationDbConnector) {
    let tenant = FoundationDbTenantName::new("kit-restore-race-test").expect("fixed tenant name");
    ensure_tenant(connector, tenant)
        .await
        .expect("provision race fixture");
    let inner = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("open race fixture");
    let transaction = inner.create_trx().expect("create clear transaction");
    let keys = metadata_keys(tenant).expect("metadata keys");
    let mut metadata = Vec::new();
    for key in &keys {
        let value = transaction
            .get(key, false)
            .await
            .expect("read metadata in clear transaction")
            .expect("metadata present");
        metadata.push((key.to_vec(), value.to_vec()));
        transaction.clear(key);
    }

    restore_metadata(connector.database(), tenant, &metadata)
        .await
        .expect("repair write must commit");
    assert!(
        transaction.commit().await.is_err(),
        "clear must conflict with the repair write that committed first"
    );
    let reopened = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("open surviving tenant");
    read_tenant_metadata(&TenantHandle::new(tenant, reopened))
        .await
        .expect("metadata remains after conflicting clear");
    delete_tenant(connector, tenant)
        .await
        .expect("clean up race fixture");
}

async fn failed_second_delete_phase_restores_metadata_after_concurrent_write(
    connector: &FoundationDbConnector,
) {
    let tenant = FoundationDbTenantName::new("kit-delete-restore-test").expect("fixed tenant name");
    ensure_tenant(connector, tenant)
        .await
        .expect("provision tenant");
    let mut metadata = Vec::new();
    clear_metadata_if_empty(connector.database(), tenant, &mut metadata)
        .await
        .expect("clear metadata phase");
    assert_eq!(metadata.len(), 2);
    let raw_tenant = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("raw handle for concurrent-write fixture");
    let transaction = raw_tenant.create_trx().expect("write transaction");
    transaction.set(b"application/data", b"value");
    transaction.commit().await.expect("concurrent write");
    assert!(matches!(
        finish_delete_after_clear(connector.database(), tenant, &metadata).await,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { .. },
        })
    ));
    assert!(connector.open_tenant(tenant).await.is_ok());
    let transaction = raw_tenant.create_trx().expect("cleanup transaction");
    transaction.clear(b"application/data");
    transaction.commit().await.expect("remove fixture data");
    drop(raw_tenant);
    delete_tenant(connector, tenant)
        .await
        .expect("delete empty tenant");
}

async fn interrupted_delete_recovery_restores_metadata_even_with_application_data(
    connector: &FoundationDbConnector,
) {
    let tenant =
        FoundationDbTenantName::new("kit-interrupted-delete-test").expect("fixed tenant name");
    ensure_tenant(connector, tenant)
        .await
        .expect("provision tenant");
    let tenant_id = TenantManagement::get_tenant(connector.database(), tenant.as_bytes())
        .await
        .expect("tenant lookup")
        .expect("tenant exists")
        .expect("tenant info")
        .id;
    let handle = connector.open_tenant(tenant).await.expect("open tenant");
    handle
        .transact_boxed(
            (),
            |transaction, _| {
                Box::pin(async move {
                    let key = TenantDataKey::new(b"application/data").expect("fixed data key");
                    transaction.set(key, b"value").expect("bounded value");
                    Ok::<(), foundationdb::FdbError>(())
                })
            },
            TenantTransactionPolicy::Write(WriteTxnPolicy::default()),
        )
        .await
        .expect("write application data");
    let raw_tenant = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("raw tenant handle for fault injection");
    let transaction = raw_tenant.create_trx().expect("fault transaction");
    for key in metadata_keys(tenant).expect("metadata keys") {
        transaction.clear(key.as_ref());
    }
    transaction.commit().await.expect("inject metadata loss");
    assert!(matches!(
        connector.open_tenant(tenant).await,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::MetadataMissing { .. }
        })
    ));
    assert!(
        recover_interrupted_delete(connector, tenant, tenant_id ^ 1)
            .await
            .is_err()
    );
    let transaction = raw_tenant.create_trx().expect("foreign metadata fixture");
    transaction.set(b"__meta/foreign", b"value");
    transaction.commit().await.expect("write foreign metadata");
    assert!(
        recover_interrupted_delete(connector, tenant, tenant_id)
            .await
            .is_err()
    );
    let transaction = raw_tenant.create_trx().expect("foreign metadata cleanup");
    transaction.clear(b"__meta/foreign");
    transaction.commit().await.expect("remove foreign metadata");
    recover_interrupted_delete(connector, tenant, tenant_id)
        .await
        .expect("recover both keys");
    assert!(connector.open_tenant(tenant).await.is_ok());
    assert!(matches!(
        delete_tenant(connector, tenant).await,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::RepairRequiresEmptyTenant { .. }
        })
    ));
    handle
        .transact_boxed(
            (),
            |transaction, _| {
                Box::pin(async move {
                    let key = TenantDataKey::new(b"application/data").expect("fixed data key");
                    transaction.clear(key);
                    Ok::<(), foundationdb::FdbError>(())
                })
            },
            TenantTransactionPolicy::Write(WriteTxnPolicy::default()),
        )
        .await
        .expect("remove application data");
    drop(handle);
    drop(raw_tenant);
    delete_tenant(connector, tenant)
        .await
        .expect("delete empty tenant");
}
