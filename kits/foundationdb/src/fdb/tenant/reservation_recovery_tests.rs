// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::Cell;

use foundationdb::tenant::TenantManagement;

use super::{
    Claim, Grant, Phase, ReservationControlTenant, ReservationErrorReason, ReservationId,
    ReservationIntent, ReservationStore, StoredReservation, TENANT_ADMIN_TIMEOUT,
    TenantDeleteResolution, configure_transaction, mapping_matches, stage_clear, stage_delete,
};
use crate::fdb::tenant::{
    FDB_NOT_COMMITTED_ERROR_CODE,
    admin::{delete_tenant, ensure_tenant, metadata_keys, tenant_id},
};
use crate::{FdbConfig, FoundationDbConnector, FoundationDbTenantName};

#[test]
fn rejects_corrupt_reservation_rows() {
    let record = StoredReservation {
        original_id: 42,
        epoch: 2,
        claimant: ReservationId::from_bytes([0xa5; 16]),
        expires_at: 10_000_000,
        phase: Phase::Recovering,
    };
    let encoded = record.encode();
    let decoded = StoredReservation::decode(&encoded).expect("valid fixture record");
    assert_eq!(decoded.original_id, record.original_id);
    assert_eq!(decoded.epoch, record.epoch);
    assert!(decoded.claimant == record.claimant);
    assert_eq!(decoded.expires_at, record.expires_at);
    assert_eq!(decoded.phase, record.phase);
    for length in 0..encoded.len() {
        assert!(matches!(
            StoredReservation::decode(&encoded[..length]),
            Err(ReservationErrorReason::InvalidRecord)
        ));
    }
    for corrupt in [
        StoredReservation {
            original_id: -1,
            ..record
        }
        .encode(),
        StoredReservation { epoch: 0, ..record }.encode(),
        StoredReservation {
            expires_at: 0,
            ..record
        }
        .encode(),
        StoredReservation {
            phase: Phase::Deleting,
            ..record
        }
        .encode(),
        [encoded.as_slice(), &[0]].concat(),
    ] {
        assert!(matches!(
            StoredReservation::decode(&corrupt),
            Err(ReservationErrorReason::InvalidRecord)
        ));
    }
}

#[tokio::test]
#[ignore = "requires a live throwaway FoundationDB cluster"]
async fn takeover_fences_retries_and_recovery_completes_atomically() {
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT * 6, verify_live_recovery())
        .await
        .expect("bounded reservation recovery test");
}

async fn verify_live_recovery() {
    let config = FdbConfig::from_env().expect("live configuration");
    // SAFETY: This live fixture initializes the native client once and
    // drops all handles before returning to the harness.
    #[allow(unsafe_code)]
    let connector =
        unsafe { FoundationDbConnector::connect(&config) }.expect("connect live cluster");
    let tenant = FoundationDbTenantName::new("kit-atomic-recovery").expect("fixture name");
    let control =
        FoundationDbTenantName::new("kit-atomic-recovery-control").expect("control fixture name");
    for name in [tenant, control] {
        assert_eq!(
            tenant_id(&connector, name).await.expect("fixture lookup"),
            None
        );
        ensure_tenant(&connector, name)
            .await
            .expect("provision fixture");
    }
    let control = ReservationControlTenant::lookup(&connector, control)
        .await
        .expect("control ID");
    let store = ReservationStore::new(&connector, control);
    let intent = ReservationIntent {
        tenant,
        original_id: tenant_id(&connector, tenant)
            .await
            .expect("target lookup")
            .expect("target ID"),
        operation: ReservationId::generate().expect("operation ID"),
    };
    let grant = store.acquire(intent).await.expect("acquire original grant");
    let competing = ReservationIntent {
        operation: ReservationId::generate().expect("competing ID"),
        ..intent
    };
    assert!(matches!(
        store.acquire(competing).await,
        Err(ReservationErrorReason::Busy)
    ));
    assert!(matches!(
        store.resume(intent).await,
        Err(ReservationErrorReason::LeaseActive)
    ));
    store
        .transact(async |transaction| {
            store.check_grant(transaction, grant).await?;
            assert!(mapping_matches(transaction, tenant, intent.original_id).await?);
            stage_clear(transaction, tenant, intent.original_id).await
        })
        .await
        .expect("commit guarded clear before the simulated interruption");

    expire(&store, intent).await;
    let recovery = Cell::new(None);
    let visits = Cell::new(0_usize);
    // Commit a takeover through a second transaction after the old delete
    // has read its grant. Its real commit conflict must re-enter the guard.
    let stale = store
        .transact(async |transaction| {
            visits.set(visits.get().checked_add(1).expect("count attempt"));
            store.check_grant(transaction, grant).await?;
            stage_delete(transaction, intent).await?;
            recovery.set(Some(claim(&store, intent).await));
            Ok(())
        })
        .await;
    assert_eq!(stale, Err(ReservationErrorReason::Stale));
    assert_eq!(visits.get(), 2);
    let first_recovery = recovery.get().expect("takeover grant");

    expire(&store, intent).await;
    let pending = connector.database().create_trx().expect("stage recovery");
    configure_transaction(&pending).expect("configure recovery");
    assert!(
        mapping_matches(&pending, control.name(), control.id())
            .await
            .is_ok_and(|matches| matches)
    );
    assert!(matches!(
        store.stage_finish(&pending, first_recovery).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    ));
    assert_observed(&store, intent, first_recovery, Phase::Recovering, false).await;

    let new_recovery = claim(&store, intent).await;
    let rejected = pending
        .commit()
        .await
        .expect_err("superseded recovery must conflict");
    assert_eq!(rejected.code(), FDB_NOT_COMMITTED_ERROR_CODE);
    assert_observed(&store, intent, new_recovery, Phase::Recovering, false).await;
    assert_eq!(
        store.finish(first_recovery).await,
        Err(ReservationErrorReason::Stale)
    );

    assert_eq!(
        store.finish(new_recovery).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert_observed(
        &store,
        intent,
        new_recovery,
        Phase::Complete(TenantDeleteResolution::MetadataSafe),
        true,
    )
    .await;
    assert_eq!(
        store.resume(intent).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    let later_delete = store
        .acquire(competing)
        .await
        .expect("acquire a later operation");
    store
        .transact(async |transaction| {
            store.check_grant(transaction, later_delete).await?;
            assert!(mapping_matches(transaction, tenant, intent.original_id).await?);
            stage_clear(transaction, tenant, intent.original_id).await
        })
        .await
        .expect("clear metadata for later delete");
    store
        .transact(async |transaction| {
            store.check_grant(transaction, later_delete).await?;
            stage_delete(transaction, competing).await
        })
        .await
        .expect("commit later deletion without recording its outcome");
    TenantManagement::create_tenant(connector.database(), tenant.as_bytes())
        .await
        .expect("reuse name after the ambiguous delete");
    let replacement = tenant_id(&connector, tenant)
        .await
        .expect("replacement lookup")
        .expect("replacement ID");
    assert_ne!(replacement, intent.original_id);
    expire(&store, competing).await;
    assert_eq!(
        store.takeover(tenant).await,
        Ok(TenantDeleteResolution::OriginalAbsent)
    );
    assert_eq!(
        tenant_id(&connector, tenant)
            .await
            .expect("replacement remains"),
        Some(replacement)
    );
    // FoundationDB will only delete the fixture if recovery left it empty.
    TenantManagement::delete_tenant(connector.database(), tenant.as_bytes())
        .await
        .expect("unchanged replacement remains empty");
    assert_eq!(
        store.resume(intent).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );

    store
        .transact(async |transaction| {
            transaction.clear(&store.record_key(tenant, intent.operation));
            transaction.clear(&store.record_key(tenant, competing.operation));
            Ok(())
        })
        .await
        .expect("clear fixture history");
    delete_tenant(&connector, control.name())
        .await
        .expect("delete control fixture");
}

async fn expire(store: &ReservationStore<'_>, intent: ReservationIntent) {
    // Move only the fixture lease deadline, using the current database read
    // version rather than waiting for a wall clock in this transaction test.
    store
        .transact(async |transaction| {
            let mut record = store
                .read_record(transaction, intent.tenant, intent.operation)
                .await?
                .expect("observed reservation");
            record.expires_at = transaction.get_read_version().await?;
            transaction.set(
                &store.record_key(intent.tenant, intent.operation),
                &record.encode(),
            );
            Ok(())
        })
        .await
        .expect("expire fixture grant");
}

async fn claim(store: &ReservationStore<'_>, intent: ReservationIntent) -> Grant {
    let result = store
        .claim_recovery(intent)
        .await
        .expect("take over expired grant");
    assert!(
        matches!(result, Claim::Acquired(_)),
        "fixture must be in flight"
    );
    match result {
        Claim::Acquired(grant) => grant,
        Claim::Complete(_) => unreachable!("asserted active fixture"),
    }
}

async fn assert_observed(
    store: &ReservationStore<'_>,
    intent: ReservationIntent,
    grant: Grant,
    phase: Phase,
    metadata_present: bool,
) {
    // Observe the control row and the target metadata in one transaction.
    store
        .transact(async |transaction| {
            let record = store
                .read_record(transaction, intent.tenant, intent.operation)
                .await?
                .expect("fixture record");
            assert_eq!(record.epoch, grant.epoch);
            assert_eq!(record.phase, phase);
            assert!(record.claimant == grant.claimant);
            let active = store.active_id(transaction, intent.tenant).await?;
            assert!(
                active
                    == if matches!(phase, Phase::Complete(_)) {
                        None
                    } else {
                        Some(intent.operation)
                    }
            );
            for key in metadata_keys(intent.tenant).expect("kit metadata keys") {
                let raw_key = [intent.original_id.to_be_bytes().as_slice(), key.as_ref()].concat();
                let value = transaction.get(&raw_key, false).await?;
                assert_eq!(value.is_some(), metadata_present);
            }
            Ok(())
        })
        .await
        .expect("observe atomic reservation and metadata state");
}
