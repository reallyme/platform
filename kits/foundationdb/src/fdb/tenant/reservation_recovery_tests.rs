// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::Cell;

use foundationdb::{Database, tenant::TenantManagement};

use super::{
    Claim, CompletionTombstone, DriverError, Grant, Phase, ReservationControlTenant,
    ReservationErrorReason, ReservationGeneration, ReservationId, ReservationIntent,
    ReservationRecord, ReservationStore, StoredReservation, TENANT_ADMIN_TIMEOUT,
    TenantDeleteResolution, configure_transaction, mapping_matches, stage_clear, stage_delete,
};
use crate::fdb::tenant::{
    FDB_NOT_COMMITTED_ERROR_CODE,
    admin::{delete_tenant, ensure_tenant, metadata_keys, tenant_id},
};
use crate::{FdbConfig, FoundationDbConnector, FoundationDbTenantName};

const FDB_COMMIT_UNKNOWN_RESULT: i32 = 1021;

#[test]
fn rejects_corrupt_reservation_rows() {
    let record = StoredReservation {
        original_id: 42,
        generation: ReservationGeneration::new(u64::MAX),
        epoch: 2,
        claimant: ReservationId::from_bytes([0xa5; 16]),
        expires_at: 10_000_000,
        phase: Phase::Recovering,
    };
    let encoded = record.encode();
    let decoded = StoredReservation::decode(&encoded).expect("valid fixture record");
    assert_eq!(decoded.original_id, record.original_id);
    assert_eq!(decoded.generation, record.generation);
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

#[test]
fn decodes_legacy_completions_and_rejects_malformed_tombstones() {
    for (tag, outcome) in [
        (3, TenantDeleteResolution::OriginalAbsent),
        (4, TenantDeleteResolution::MetadataSafe),
    ] {
        // A fixed v1 fixture guards the wire representation independently of
        // the current encoder. Its terminal record still contains a grant.
        let mut legacy = [0; 42];
        legacy[0] = 1;
        legacy[1] = tag;
        legacy[9] = 42;
        legacy[17] = 1;
        legacy[41] = 1;
        assert!(matches!(
            ReservationRecord::decode(&legacy),
            Ok(ReservationRecord::Full(StoredReservation {
                original_id: 42,
                phase: Phase::Complete(saved),
                ..
            })) if saved == outcome
        ));
        let compact = CompletionTombstone {
            original_id: 42,
            generation: ReservationGeneration::LEGACY,
            outcome,
        }
        .encode();
        assert_eq!(compact, [2, tag, 0, 0, 0, 0, 0, 0, 0, 42]);
        assert!(matches!(
            ReservationRecord::decode(&compact),
            Ok(ReservationRecord::Tombstone(CompletionTombstone {
                original_id: 42,
                generation: ReservationGeneration::LEGACY,
                outcome: saved,
            })) if saved == outcome
        ));
        for length in 0..compact.len() {
            assert!(matches!(
                ReservationRecord::decode(&compact[..length]),
                Err(ReservationErrorReason::InvalidRecord)
            ));
        }
        let mut negative_id = compact.clone();
        negative_id[2] = u8::MAX;
        let mut active_tag = compact.clone();
        active_tag[1] = 1;
        let mut unknown_version = compact.clone();
        unknown_version[0] = u8::MAX;
        for malformed in [
            negative_id,
            active_tag,
            unknown_version,
            [compact.as_slice(), &[0]].concat(),
        ] {
            assert!(matches!(
                ReservationRecord::decode(&malformed),
                Err(ReservationErrorReason::InvalidRecord)
            ));
        }
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
        generation: ReservationGeneration::LEGACY,
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

#[tokio::test]
#[ignore = "requires a live throwaway FoundationDB cluster"]
async fn prune_retries_after_lost_reply_and_fences_stale_resumes() {
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT * 6, verify_live_pruning())
        .await
        .expect("bounded reservation pruning test");
}

async fn verify_live_pruning() {
    let config = FdbConfig::from_env().expect("live configuration");
    // SAFETY: The connector owns the native client for the entire fixture.
    // The second database handle is dropped before the client shuts down.
    #[allow(unsafe_code)]
    let connector =
        unsafe { FoundationDbConnector::connect(&config) }.expect("connect live cluster");
    let peer_database = Database::new(config.fdb_cluster_file()).expect("second client");
    let tenant = FoundationDbTenantName::new("kit-prune-reservation").expect("fixture name");
    let control_name =
        FoundationDbTenantName::new("kit-prune-reservation-control").expect("control name");
    for name in [tenant, control_name] {
        assert_eq!(
            tenant_id(&connector, name).await.expect("fixture lookup"),
            None
        );
        ensure_tenant(&connector, name)
            .await
            .expect("provision fixture");
    }
    let control = ReservationControlTenant::lookup(&connector, control_name)
        .await
        .expect("control ID");
    let store = ReservationStore::new(&connector, control);
    let peer = ReservationStore {
        database: &peer_database,
        control,
    };
    let old = ReservationIntent {
        tenant,
        original_id: tenant_id(&connector, tenant)
            .await
            .expect("target lookup")
            .expect("target ID"),
        operation: ReservationId::generate().expect("old operation ID"),
        generation: ReservationGeneration::LEGACY,
    };
    let absent = TenantDeleteResolution::OriginalAbsent;
    assert_eq!(
        store.prune_completed(old).await,
        Err(ReservationErrorReason::PruneNotCompleted)
    );
    assert_eq!(store.delete(old).await, Ok(absent));

    TenantManagement::create_tenant(&peer_database, tenant.as_bytes())
        .await
        .expect("reuse the name for an empty replacement");
    let replacement = ReservationIntent {
        tenant,
        original_id: tenant_id(&connector, tenant)
            .await
            .expect("replacement lookup")
            .expect("replacement ID"),
        operation: ReservationId::generate().expect("replacement operation ID"),
        generation: ReservationGeneration::LEGACY,
    };
    assert_ne!(old.original_id, replacement.original_id);
    let newer_grant = peer
        .acquire(replacement)
        .await
        .expect("grant for replacement");
    assert_eq!(
        store.prune_completed(replacement).await,
        Err(ReservationErrorReason::PruneNotCompleted)
    );
    let wrong_id = ReservationIntent {
        original_id: replacement.original_id,
        ..old
    };
    assert_eq!(
        store.prune_completed(wrong_id).await,
        Err(ReservationErrorReason::InvalidRecord)
    );

    let row = store.record_key(tenant, old.operation);
    let attempts = Cell::new(0_u8);
    let retried = store
        .transact(async |transaction| {
            attempts.set(attempts.get().checked_add(1).expect("count prune attempts"));
            let outcome = store.stage_prune(transaction, old).await?;
            if attempts.get() == 1 {
                peer.transact(async |independent| {
                    peer.check_grant(independent, newer_grant).await?;
                    let value = independent
                        .get(&row, false)
                        .await?
                        .expect("full completion row");
                    assert_eq!(value.len(), 42);
                    Ok(())
                })
                .await
                .expect("the staged prune is invisible");
                // Commit the staged operation through a second client, then
                // deliver an unknown-result error to the first client's retry loop.
                assert_eq!(peer.prune_completed(old).await, Ok(absent));
                return Err(DriverError::from_code(FDB_COMMIT_UNKNOWN_RESULT).into());
            }
            Ok(outcome)
        })
        .await;
    assert_eq!(retried, Ok(absent));
    assert_eq!(attempts.get(), 2);
    assert_eq!(
        store.prune_completed(wrong_id).await,
        Err(ReservationErrorReason::InvalidRecord)
    );
    assert_eq!(
        store.resume(wrong_id).await,
        Err(ReservationErrorReason::InvalidRecord)
    );

    let reader = peer_database
        .create_trx()
        .expect("create tombstone observer");
    configure_transaction(&reader).expect("configure tombstone observer");
    let compact = reader
        .get(&row, false)
        .await
        .expect("read completion")
        .expect("same operation key");
    assert_eq!(compact.len(), 10);
    assert!(matches!(
        ReservationRecord::decode(compact.as_ref()),
        Ok(ReservationRecord::Tombstone(CompletionTombstone { original_id, outcome, generation: ReservationGeneration::LEGACY }))
            if original_id == old.original_id && outcome == absent
    ));
    assert_eq!(store.prune_completed(old).await, Ok(absent));
    assert_eq!(store.resume(old).await, Ok(absent));
    assert!(matches!(
        store.acquire(old).await,
        Err(ReservationErrorReason::Stale)
    ));
    // A write gives the observer's non-snapshot read a commit-time conflict
    // check. It would fail if any retry had rewritten the tombstone.
    let probe = [
        control.id().to_be_bytes().as_slice(),
        b"prune-read-conflict-probe",
    ]
    .concat();
    reader.set(&probe, b"observed");
    reader
        .commit()
        .await
        .expect("terminal replay leaves the row unchanged");
    assert_observed(&peer, replacement, newer_grant, Phase::Deleting, false).await;

    expire(&peer, replacement).await;
    assert_eq!(
        peer.resume(replacement).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert_eq!(store.resume(old).await, Ok(absent));
    store
        .transact(async |transaction| {
            assert!(store.active_id(transaction, tenant).await?.is_none());
            assert!(mapping_matches(transaction, tenant, replacement.original_id).await?);
            assert!(matches!(
                store
                    .read_record(transaction, tenant, replacement.operation)
                    .await?,
                Some(ReservationRecord::Full(StoredReservation {
                    phase: Phase::Complete(TenantDeleteResolution::MetadataSafe),
                    ..
                }))
            ));
            transaction.clear(&row);
            transaction.clear(&store.record_key(tenant, replacement.operation));
            transaction.clear(&probe);
            Ok(())
        })
        .await
        .expect("verify replacement and clear fixture history");
    delete_tenant(&connector, tenant)
        .await
        .expect("delete replacement fixture");
    delete_tenant(&connector, control_name)
        .await
        .expect("delete control fixture");
}

async fn expire(store: &ReservationStore<'_>, intent: ReservationIntent) {
    // Move only the fixture lease deadline, using the current database read
    // version rather than waiting for a wall clock in this transaction test.
    store
        .transact(async |transaction| {
            let Some(ReservationRecord::Full(mut record)) = store
                .read_record(transaction, intent.tenant, intent.operation)
                .await?
            else {
                return Err(ReservationErrorReason::InvalidRecord.into());
            };
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
            let Some(ReservationRecord::Full(record)) = store
                .read_record(transaction, intent.tenant, intent.operation)
                .await?
            else {
                return Err(ReservationErrorReason::InvalidRecord.into());
            };
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
