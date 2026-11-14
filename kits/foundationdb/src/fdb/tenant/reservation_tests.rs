// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Live transaction-layout check for cluster-side tenant reservations.

use std::time::Duration;

use foundationdb::{Database, RangeOption, Transaction, tenant::TenantInfo};
use tokio::process::Command;

use super::{
    DELETE_SCAN_LIMIT, TENANT_ADMIN_TIMEOUT, TENANT_ADMIN_TIMEOUT_MILLIS, TenantMapDeleteOutcome,
    TransactionOption, delete_tenant, ensure_tenant, metadata_keys, stage_tenant_map_delete,
    tenant_id, tenant_map_key,
};
use crate::fdb::{
    config::FdbConfig, connector::FoundationDbConnector, tenant::FDB_NOT_COMMITTED_ERROR_CODE,
    tenant_name::FoundationDbTenantName,
};

const TENANT_NAME: &str = "kit-reservation-conflict-test";
const CONTROL_TENANT_NAME: &str = "kit-reservation-control-test";
// The reservation must outlive the target tenant. Required tenant mode
// rejects raw writes outside a known tenant prefix.
const RESERVATION_KEY: &[u8] = b"conformance/tenant-reservation/kit-reservation-conflict-test";
const PEER_TEST: &str = "fdb::tenant::admin::reservation_tests::reservation_peer";
const PEER_EPOCH: &str = "FDB_RESERVATION_TEST_EXPECTED_EPOCH";
const PEER_DEADLINE: Duration = Duration::from_secs(4);

fn connect() -> FoundationDbConnector {
    let config = FdbConfig::from_env().expect("live FoundationDB configuration");
    // SAFETY: Each test process initializes the client once and drops its
    // connector and transactions before returning from the test harness.
    #[allow(unsafe_code)]
    unsafe { FoundationDbConnector::connect(&config) }.expect("live FoundationDB connector")
}

fn cluster_transaction(database: &Database) -> Transaction {
    let transaction = database.create_trx().expect("create cluster transaction");
    transaction
        .set_option(TransactionOption::RawAccess)
        .expect("enable access to cluster and tenant-prefixed keys");
    transaction
        .set_option(TransactionOption::Timeout(TENANT_ADMIN_TIMEOUT_MILLIS))
        .expect("bound transaction lifetime");
    transaction
}

async fn read_reservation(transaction: &Transaction, key: &[u8], expected: u64) {
    let value = transaction
        .get(key, false)
        .await
        .expect("read reservation with a conflict range")
        .expect("reservation exists");
    assert_eq!(value.as_ref(), &expected.to_be_bytes());
}

#[tokio::test]
#[ignore = "subprocess fixture for the live reservation regression"]
async fn reservation_peer() {
    let expected = std::env::var(PEER_EPOCH)
        .expect("fixture epoch")
        .parse::<u64>()
        .expect("parse fixture epoch");
    let next = expected.checked_add(1).expect("fixture epoch can advance");
    let connector = connect();
    let handle = connector
        .database()
        .open_tenant(CONTROL_TENANT_NAME.as_bytes())
        .expect("open control tenant");
    let transaction = handle
        .create_trx()
        .expect("create control tenant transaction");
    transaction
        .set_option(TransactionOption::Timeout(TENANT_ADMIN_TIMEOUT_MILLIS))
        .expect("bound peer transaction");
    read_reservation(&transaction, RESERVATION_KEY, expected).await;
    transaction.set(RESERVATION_KEY, &next.to_be_bytes());
    transaction
        .commit()
        .await
        .expect("commit reservation takeover");
}

async fn advance_from_peer(expected: u64) {
    let mut peer = Command::new(std::env::current_exe().expect("test executable"));
    peer.args(["--ignored", "--exact", PEER_TEST])
        .env(PEER_EPOCH, expected.to_string())
        .kill_on_drop(true);
    let output = tokio::time::timeout(PEER_DEADLINE, peer.output())
        .await
        .expect("second client finished before transaction deadline")
        .expect("start second FoundationDB client");
    assert!(
        output.status.success(),
        "peer failed: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
}

async fn guarded_transaction(database: &Database, key: &[u8], epoch: u64) -> Transaction {
    let transaction = cluster_transaction(database);
    transaction
        .set_option(TransactionOption::ReportConflictingKeys)
        .expect("report the actual conflicting keys");
    read_reservation(&transaction, key, epoch).await;
    transaction
}

fn tenant_prefix(id: i64) -> [u8; 8] {
    // FoundationDB 7.3 uses an eight-byte big-endian tenant ID as the prefix.
    u64::try_from(id)
        .expect("nonnegative tenant ID")
        .to_be_bytes()
}

async fn stage_raw_metadata_clear(
    transaction: &Transaction,
    tenant: FoundationDbTenantName,
    original_id: i64,
) {
    transaction
        .set_option(TransactionOption::ReadSystemKeys)
        .expect("enable tenant-map lookup");
    let map_key = tenant_map_key(tenant).expect("tenant-map key");
    let value = transaction
        .get(&map_key, false)
        .await
        .expect("read tenant mapping in the guarded transaction")
        .expect("tenant mapping exists");
    let mapping = TenantInfo::try_from((map_key.as_slice(), value.as_ref()))
        .expect("decode the tenant mapping");
    assert_eq!(mapping.id, original_id);

    // Check what the native tenant handle actually wrote before staging any
    // raw clears. This also verifies the prefix matches the target keyspace.
    let prefix = tenant_prefix(mapping.id);
    let end = [prefix.as_slice(), b"\xff"].concat();
    let mut range = RangeOption::from((prefix.as_slice(), end.as_slice()));
    range.limit = Some(usize::try_from(DELETE_SCAN_LIMIT).expect("positive metadata scan limit"));
    let values = transaction
        .get_range(&range, 1, false)
        .await
        .expect("read tenant data with a conflict range");
    let [schema, created_at] = metadata_keys(tenant).expect("kit metadata keys");
    let keys = [schema, created_at].map(|key| [prefix.as_slice(), key.as_ref()].concat());
    assert!(!values.more());
    assert_eq!(values.len(), keys.len());
    for key in &keys {
        assert!(values.iter().any(|entry| entry.key() == key));
        transaction.clear(key);
    }
}

async fn assert_reservation_conflict(transaction: Transaction, key: &[u8]) {
    let error = transaction
        .commit()
        .await
        .expect_err("obsolete reservation epoch must prevent the commit");
    assert_eq!(error.code(), FDB_NOT_COMMITTED_ERROR_CODE);
    let ranges = error
        .conflicting_keys()
        .await
        .expect("resolver conflict ranges");
    assert!(
        ranges
            .iter()
            .any(|range| range.begin() <= key && key < range.end()),
        "FoundationDB must identify the reservation read as the cause of the conflict",
    );
}

#[tokio::test]
#[ignore = "requires a live throwaway FoundationDB cluster"]
async fn reservation_takeover_conflicts_with_clear_and_delete() {
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT * 6, verify_reservation_conflicts())
        .await
        .expect("live reservation regression completed");
}

async fn verify_reservation_conflicts() {
    let connector = connect();
    let database = connector.database();
    let tenant = FoundationDbTenantName::new(TENANT_NAME).expect("fixture tenant name");
    let control = FoundationDbTenantName::new(CONTROL_TENANT_NAME).expect("control tenant name");
    let original_id = provision_fixture(&connector, tenant).await;
    let control_id = provision_fixture(&connector, control).await;
    let reservation_key = [tenant_prefix(control_id).as_slice(), RESERVATION_KEY].concat();
    let map_key = tenant_map_key(tenant).expect("tenant-map key");

    let seed = cluster_transaction(database);
    assert!(
        seed.get(&reservation_key, false)
            .await
            .expect("check fixture reservation")
            .is_none()
    );
    let initial_epoch = 1_u64;
    seed.set(&reservation_key, &initial_epoch.to_be_bytes());
    seed.commit().await.expect("commit initial reservation");

    let stale_clear = guarded_transaction(database, &reservation_key, initial_epoch).await;
    stage_raw_metadata_clear(&stale_clear, tenant, original_id).await;
    advance_from_peer(initial_epoch).await;
    assert_reservation_conflict(stale_clear, &reservation_key).await;
    connector
        .open_tenant(tenant)
        .await
        .expect("rejected clear leaves metadata readable");

    let next_epoch = initial_epoch
        .checked_add(1)
        .expect("fixture epoch advances");
    let current_clear = guarded_transaction(database, &reservation_key, next_epoch).await;
    stage_raw_metadata_clear(&current_clear, tenant, original_id).await;
    current_clear
        .commit()
        .await
        .expect("same layout can clear metadata with a current grant");

    let stale_delete = guarded_transaction(database, &reservation_key, next_epoch).await;
    assert_eq!(
        stage_tenant_map_delete(&stale_delete, &map_key, original_id)
            .await
            .expect("stage tenant-map delete"),
        TenantMapDeleteOutcome::ClearStaged,
    );
    advance_from_peer(next_epoch).await;
    assert_reservation_conflict(stale_delete, &reservation_key).await;
    assert_eq!(
        tenant_id(&connector, tenant)
            .await
            .expect("read surviving mapping"),
        Some(original_id)
    );

    let final_epoch = next_epoch.checked_add(1).expect("fixture epoch advances");
    let current_delete = guarded_transaction(database, &reservation_key, final_epoch).await;
    assert_eq!(
        stage_tenant_map_delete(&current_delete, &map_key, original_id)
            .await
            .expect("stage current delete"),
        TenantMapDeleteOutcome::ClearStaged,
    );
    current_delete
        .commit()
        .await
        .expect("same layout can delete the empty tenant with a current grant");
    assert_eq!(
        tenant_id(&connector, tenant)
            .await
            .expect("read deleted mapping"),
        None
    );

    let cleanup = guarded_transaction(database, &reservation_key, final_epoch).await;
    cleanup.clear(&reservation_key);
    cleanup.commit().await.expect("remove fixture reservation");
    delete_tenant(&connector, control)
        .await
        .expect("delete control tenant");
}

async fn provision_fixture(
    connector: &FoundationDbConnector,
    tenant: FoundationDbTenantName,
) -> i64 {
    assert_eq!(
        tenant_id(connector, tenant)
            .await
            .expect("look up fixture name"),
        None
    );
    ensure_tenant(connector, tenant)
        .await
        .expect("provision fixture through the native tenant handle");
    tenant_id(connector, tenant)
        .await
        .expect("look up fixture ID")
        .expect("fixture exists")
}
