// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::fdb::tenant_name::FoundationDbTenantName;

use super::metadata_keys;
use super::{
    TENANT_ADMIN_TIMEOUT, TENANT_ADMIN_TIMEOUT_MILLIS, administration_failed, delete_tenant,
    ensure_tenant, finish_delete_after_clear, open_delete_tenant, recover_interrupted_delete,
    repair_absent_tenant_metadata, repair_tenant_metadata, restore_metadata_with_retries,
    stage_metadata_clear, tenant_id,
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
    tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT * 4,
        recovery_stays_bound_when_a_tenant_name_is_reused(&connector),
    )
    .await
    .expect("name reuse fixture completed");
    tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT * 5,
        automatic_restore_does_not_write_to_a_replacement(&connector),
    )
    .await
    .expect("automatic restore fixture completed");
    restore_write_conflicts_with_in_flight_clear(&connector).await;
    interrupted_creation_requires_explicit_empty_tenant_repair(&connector).await;
}

async fn recovery_stays_bound_when_a_tenant_name_is_reused(connector: &FoundationDbConnector) {
    let tenant =
        FoundationDbTenantName::new("kit-reused-recovery-test").expect("fixed tenant name");
    ensure_tenant(connector, tenant)
        .await
        .expect("provision original");
    let recorded_id = tenant_id(connector, tenant)
        .await
        .expect("look up original tenant ID")
        .expect("original tenant exists");
    let verified_original = tokio::time::timeout(
        TENANT_ADMIN_TIMEOUT,
        connector
            .recovery_database()
            .open_tenant(tenant)
            .expect("open original recovery handle")
            .verify(recorded_id),
    )
    .await
    .expect("resolve original handle ID within deadline")
    .expect("original handle matches recorded ID");
    let (transaction, _) = stage_metadata_clear(&verified_original)
        .await
        .expect("stage the first phase of deletion");
    transaction
        .commit()
        .await
        .expect("complete the first phase of deletion");

    TenantManagement::delete_tenant(connector.database(), tenant.as_bytes())
        .await
        .expect("delete original after recovery verified its handle");
    TenantManagement::create_tenant(connector.database(), tenant.as_bytes())
        .await
        .expect("recreate name before recovery attempts its write");
    let replacement_id = tenant_id(connector, tenant)
        .await
        .expect("look up replacement tenant ID")
        .expect("replacement tenant exists");
    assert_ne!(recorded_id, replacement_id);
    let replacement = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("open replacement tenant");

    assert!(
        repair_absent_tenant_metadata(&verified_original, i64::from(TENANT_ADMIN_TIMEOUT_MILLIS))
            .await
            .is_err(),
        "a transaction from the verified handle must fail after that tenant is deleted"
    );
    assert!(matches!(
        recover_interrupted_delete(connector, tenant, recorded_id).await,
        Err(FdbError::Tenant {
            reason: TenantErrorReason::AdministrationFailed { .. }
        })
    ));
    let transaction = replacement.create_trx().expect("create replacement read");
    for key in metadata_keys(tenant).expect("metadata keys") {
        assert!(
            transaction
                .get(&key, false)
                .await
                .expect("read replacement metadata")
                .is_none(),
            "neither stale recovery path may initialize the replacement"
        );
    }
    drop(transaction);

    recover_interrupted_delete(connector, tenant, replacement_id)
        .await
        .expect("the current recorded ID can recover its own tenant");
    connector
        .open_tenant(tenant)
        .await
        .expect("recovered metadata is compatible");
    delete_tenant(connector, tenant)
        .await
        .expect("clean up replacement");
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
    let handle = open_delete_tenant(connector, tenant)
        .await
        .expect("verified tenant");
    let (transaction, snapshot) = stage_metadata_clear(&handle)
        .await
        .expect("stage metadata clear");

    restore_metadata_with_retries(connector, &snapshot)
        .await
        .expect("repair write must commit");
    assert!(
        transaction.commit().await.is_err(),
        "clear must conflict with the repair write that committed first"
    );
    connector
        .open_tenant(tenant)
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
    let handle = open_delete_tenant(connector, tenant)
        .await
        .expect("verified tenant");
    let (transaction, snapshot) = stage_metadata_clear(&handle)
        .await
        .expect("stage metadata clear");
    transaction.commit().await.expect("clear metadata phase");
    assert_eq!(snapshot.entries.len(), 2);
    let raw_tenant = connector
        .database()
        .open_tenant(tenant.as_bytes())
        .expect("raw handle for concurrent-write fixture");
    let transaction = raw_tenant.create_trx().expect("write transaction");
    transaction.set(b"application/data", b"value");
    transaction.commit().await.expect("concurrent write");
    assert!(matches!(
        finish_delete_after_clear(connector, &snapshot).await,
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

async fn automatic_restore_does_not_write_to_a_replacement(connector: &FoundationDbConnector) {
    let tenant = FoundationDbTenantName::new("kit-reused-restore-test").expect("fixed tenant name");
    ensure_tenant(connector, tenant)
        .await
        .expect("provision original");
    let original = open_delete_tenant(connector, tenant)
        .await
        .expect("verified original");
    let (transaction, snapshot) = stage_metadata_clear(&original)
        .await
        .expect("snapshot original metadata");
    transaction.commit().await.expect("clear original metadata");
    assert_eq!(snapshot.entries.len(), 2);

    // Commit the tenant-map delete without delivering its result to the
    // restore path, then reuse the name before any restore attempt opens it.
    let database = connector.database();
    TenantManagement::delete_tenant(database, tenant.as_bytes())
        .await
        .expect("commit original tenant deletion");
    TenantManagement::create_tenant(database, tenant.as_bytes())
        .await
        .expect("recreate the same name");
    let replacement_id = tenant_id(connector, tenant)
        .await
        .expect("lookup succeeds")
        .expect("replacement exists");
    assert_ne!(replacement_id, snapshot.original_id);
    let replacement = database
        .open_tenant(tenant.as_bytes())
        .expect("open replacement");
    let [schema, created_at] = metadata_keys(tenant).expect("metadata keys");
    let marker = 9_u32.to_le_bytes();
    let transaction = replacement
        .create_trx()
        .expect("replacement fixture transaction");
    transaction.set(&schema, &marker);
    transaction
        .commit()
        .await
        .expect("write replacement metadata");

    assert_eq!(
        restore_metadata_with_retries(connector, &snapshot).await,
        Err(administration_failed(tenant)),
        "every retry must reject the replacement ID",
    );
    let transaction = replacement
        .create_trx()
        .expect("verify replacement metadata");
    let schema_value = transaction
        .get(&schema, false)
        .await
        .expect("read schema key")
        .expect("replacement schema key remains");
    assert_eq!(schema_value.as_ref(), &marker);
    assert!(
        transaction
            .get(&created_at, false)
            .await
            .expect("read creation time")
            .is_none()
    );
    drop(transaction);
    delete_tenant(connector, tenant)
        .await
        .expect("clean up replacement");
}

async fn interrupted_delete_recovery_restores_metadata_even_with_application_data(
    connector: &FoundationDbConnector,
) {
    let tenant =
        FoundationDbTenantName::new("kit-interrupted-delete-test").expect("fixed tenant name");
    ensure_tenant(connector, tenant)
        .await
        .expect("provision tenant");
    let tenant_id = tenant_id(connector, tenant)
        .await
        .expect("tenant ID lookup")
        .expect("tenant exists");
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
