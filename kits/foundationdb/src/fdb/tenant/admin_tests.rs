// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::fdb::tenant_name::FoundationDbTenantName;

use super::metadata_keys;
use super::{
    clear_metadata_if_empty, delete_tenant, ensure_tenant, finish_delete_after_clear,
    recover_interrupted_delete,
};
use crate::fdb::config::FdbConfig;
use crate::fdb::connector::FoundationDbConnector;
use crate::fdb::error::{FdbError, TenantErrorReason};
use crate::fdb::tenant::TenantDataKey;
use crate::fdb::transaction::{TenantTransactionPolicy, WriteTxnPolicy};

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
    recover_interrupted_delete(connector, tenant)
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
                    transaction.clear(key).expect("write transaction");
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
