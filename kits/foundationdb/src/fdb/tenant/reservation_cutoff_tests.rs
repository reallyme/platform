// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::Cell;

use foundationdb::Database;

use super::{
    Claim, DriverError, ReservationControlTenant, ReservationErrorReason, ReservationGeneration,
    ReservationId, ReservationIntent, ReservationStore, TENANT_ADMIN_TIMEOUT,
    TenantDeleteResolution,
};
use crate::fdb::tenant::admin::{delete_tenant, ensure_tenant};
use crate::{FdbConfig, FoundationDbConnector, FoundationDbTenantName};

const FDB_COMMIT_UNKNOWN_RESULT: i32 = 1021;

#[tokio::test]
#[ignore = "requires a live throwaway FoundationDB cluster"]
async fn cutoff_advances_serialize_with_claims_and_retries() {
    tokio::time::timeout(TENANT_ADMIN_TIMEOUT * 6, verify_cutoff())
        .await
        .expect("bounded cutoff regression");
}

async fn verify_cutoff() {
    let config = FdbConfig::from_env().expect("live FoundationDB configuration");
    // SAFETY: The connector owns the native client. All database handles
    // are dropped before the connector shuts the client down.
    #[allow(unsafe_code)]
    let connector = unsafe { FoundationDbConnector::connect(&config) }.expect("live cluster");
    let peer_db = Database::new(config.fdb_cluster_file()).expect("independent client handle");
    let tenant = FoundationDbTenantName::new("kit-admission-cutoff-test").expect("fixture name");
    let control_name =
        FoundationDbTenantName::new("kit-admission-cutoff-control").expect("control name");
    for name in [tenant, control_name] {
        assert_eq!(
            super::tenant_id(&connector, name)
                .await
                .expect("verify fixture"),
            None
        );
        ensure_tenant(&connector, name)
            .await
            .expect("provision fixture");
    }
    let control = ReservationControlTenant::lookup(&connector, control_name)
        .await
        .expect("control identity");
    let store = ReservationStore::new(&connector, control);
    let peer = ReservationStore {
        database: &peer_db,
        control,
    };
    let legacy = ReservationGeneration::LEGACY;
    let first = ReservationGeneration::new(1);
    let second = ReservationGeneration::new(2);
    let third = ReservationGeneration::new(3);
    assert_eq!(store.cutoff(tenant).await, Ok(legacy));
    for invalid in [legacy, second, ReservationGeneration::new(u64::MAX)] {
        assert_eq!(
            store.advance_cutoff(tenant, invalid).await,
            Err(ReservationErrorReason::InvalidCutoffTarget)
        );
    }

    let historical = capture(&store, tenant, legacy).await;
    assert_eq!(
        store.resume(historical).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert_eq!(
        store.prune_completed(historical).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert_eq!(
        store.reclaim_tombstone(historical).await,
        Err(ReservationErrorReason::ReclaimNotEligible)
    );
    let old = capture(&store, tenant, legacy).await;
    let visits = Cell::new(0_u8);
    let result = store
        .transact(async |transaction| {
            visits.set(visits.get().checked_add(1).expect("count claim attempts"));
            let grant = store.stage_acquire(transaction, old).await?;
            // A second client advances while this claim holds the old read
            // version. The commit conflict must retry the cutoff check.
            assert_eq!(peer.advance_cutoff(tenant, first).await, Ok(first));
            Ok(grant)
        })
        .await;
    assert!(matches!(result, Err(ReservationErrorReason::IntentExpired)));
    assert_eq!(visits.get(), 2);
    assert_eq!(
        store.resume(old).await,
        Err(ReservationErrorReason::IntentExpired)
    );
    assert_eq!(
        store.resume(historical).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );

    let current = capture(&store, tenant, first).await;
    let winner = Cell::new(None);
    visits.set(0);
    let result = store
        .transact(async |transaction| {
            visits.set(visits.get().checked_add(1).expect("count advance attempts"));
            let generation = store
                .stage_advance_cutoff(transaction, tenant, second)
                .await?;
            // Reverse the commit order. An advance that loses to the claim
            // sees the active slot on retry and keeps the cutoff unchanged.
            winner.set(Some(
                peer.acquire(current)
                    .await
                    .expect("claim before cutoff commit"),
            ));
            Ok(generation)
        })
        .await;
    assert_eq!(result, Err(ReservationErrorReason::Busy));
    assert_eq!(visits.get(), 2);
    assert_eq!(store.cutoff(tenant).await, Ok(first));
    assert_eq!(store.advance_cutoff(tenant, first).await, Ok(first));

    let grant = winner.get().expect("winning grant");
    store
        .transact(async |transaction| {
            store.check_grant(transaction, grant).await?;
            super::stage_clear(transaction, tenant, current.original_id).await
        })
        .await
        .expect("clear fixture metadata");
    store
        .transact(async |transaction| {
            store.check_grant(transaction, grant).await?;
            super::stage_delete(transaction, current).await
        })
        .await
        .expect("delete original fixture");
    assert_eq!(
        store.finish(grant).await,
        Ok(TenantDeleteResolution::OriginalAbsent)
    );
    ensure_tenant(&connector, tenant)
        .await
        .expect("reuse fixture name");

    let interrupted = capture(&store, tenant, first).await;
    assert_ne!(interrupted.original_id, old.original_id);
    let claimant = ReservationId::generate().expect("reserve recovery identity");
    visits.set(0);
    let result = store
        .transact(async |transaction| {
            visits.set(visits.get().checked_add(1).expect("count resume attempts"));
            let claim = store
                .stage_claim_recovery(transaction, interrupted, claimant)
                .await?;
            assert_eq!(peer.advance_cutoff(tenant, second).await, Ok(second));
            Ok(claim)
        })
        .await;
    assert!(matches!(result, Err(ReservationErrorReason::IntentExpired)));
    assert_eq!(visits.get(), 2);

    visits.set(0);
    let result = store
        .transact(async |transaction| {
            visits.set(
                visits
                    .get()
                    .checked_add(1)
                    .expect("count ambiguous advance attempts"),
            );
            let generation = store
                .stage_advance_cutoff(transaction, tenant, third)
                .await?;
            if visits.get() == 1 {
                assert_eq!(peer.advance_cutoff(tenant, third).await, Ok(third));
                return Err(DriverError::from_code(FDB_COMMIT_UNKNOWN_RESULT).into());
            }
            Ok(generation)
        })
        .await;
    assert_eq!(result, Ok(third));
    assert_eq!(visits.get(), 2);
    let fresh = capture(&peer, tenant, third).await;
    assert_eq!(fresh.original_id, interrupted.original_id);
    let fresh_grant = match store.claim_recovery(fresh).await.expect("fresh claim") {
        Claim::Acquired(grant) => Some(grant),
        Claim::Complete(_) => None,
    }
    .expect("first claim acquires a grant");
    assert_eq!(
        store.reclaim_tombstone(current).await,
        Err(ReservationErrorReason::ReclaimNotEligible)
    );
    let wrong_id = ReservationIntent {
        original_id: fresh.original_id,
        ..historical
    };
    assert_eq!(
        store.reclaim_tombstone(wrong_id).await,
        Err(ReservationErrorReason::InvalidRecord)
    );
    visits.set(0);
    let result = store
        .transact(async |transaction| {
            visits.set(
                visits
                    .get()
                    .checked_add(1)
                    .expect("count ambiguous reclaim attempts"),
            );
            store.stage_reclaim(transaction, historical).await?;
            if visits.get() == 1 {
                peer.reclaim_tombstone(historical)
                    .await
                    .expect("commit reclaim through second client");
                return Err(DriverError::from_code(FDB_COMMIT_UNKNOWN_RESULT).into());
            }
            Ok(())
        })
        .await;
    assert_eq!(result, Ok(()));
    assert_eq!(visits.get(), 2);
    assert_eq!(
        store.resume(historical).await,
        Err(ReservationErrorReason::IntentExpired)
    );
    assert_eq!(
        store.finish(fresh_grant).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert!(matches!(
        store.acquire(historical).await,
        Err(ReservationErrorReason::IntentExpired)
    ));
    assert_eq!(
        store.prune_completed(fresh).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert_eq!(
        store.resume(fresh).await,
        Ok(TenantDeleteResolution::MetadataSafe)
    );
    assert_eq!(store.advance_cutoff(tenant, first).await, Ok(third));
    store
        .transact(async |transaction| {
            assert!(store.active_id(transaction, tenant).await?.is_none());
            for rejected in [old, interrupted, historical] {
                assert!(
                    store
                        .read_record(transaction, tenant, rejected.operation)
                        .await?
                        .is_none()
                );
            }
            for completed in [current, fresh] {
                transaction.clear(&store.record_key(tenant, completed.operation));
            }
            transaction.clear(&store.cutoff_key(tenant));
            Ok(())
        })
        .await
        .expect("verify state and clear fixture history");
    delete_tenant(&connector, tenant)
        .await
        .expect("clear replacement fixture");
    delete_tenant(&connector, control_name)
        .await
        .expect("clear control fixture");
}

async fn capture(
    store: &ReservationStore<'_>,
    tenant: FoundationDbTenantName,
    generation: ReservationGeneration,
) -> ReservationIntent {
    let intent = store
        .capture_intent(tenant)
        .await
        .expect("capture succeeds")
        .expect("tenant present");
    assert_eq!(intent.generation, generation);
    intent
}
