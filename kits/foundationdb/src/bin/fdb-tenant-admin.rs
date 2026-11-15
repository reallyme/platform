// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Standalone tenant operations for FoundationDB operator workflows.
//!
//! Invocations:
//! - `ensure <tenant-name>`
//! - `repair <tenant-name>`
//! - `recover-delete <tenant-name> <expected-tenant-id>`
//! - `delete <tenant-name>`
//! - `delete <tenant-name> --record <path>`
//! - `resume-delete <path>`
//! - `prune-delete <path>`
//! - `reclaim-delete <path>`
//! - `takeover-delete <tenant-name>`
//! - `delete-cutoff <tenant-name>`
//! - `advance-delete-cutoff <tenant-name> <target-generation>`
//! - `exists <tenant-name>`
//! - `id <tenant-name>`

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use admin::reservations::{
    ReservationControlTenant, ReservationErrorReason, ReservationGeneration, ReservationIntent,
    ReservationStore,
};
use reallyme_foundationdb_kit::FoundationDbTenantName;
use reallyme_foundationdb_kit::{FdbConfig, FdbContext, fdb::tenant::admin};
use thiserror::Error;

#[path = "fdb-tenant-admin/delete_record.rs"]
mod delete_record;
use delete_record::{
    DeleteRecord, DeleteRecordError, DeleteRecordState, DeleteReservation, LockedDeleteRecord,
};

const USAGE: &str = "usage: fdb-tenant-admin <ensure|repair|delete|exists|id|takeover-delete|delete-cutoff> <tenant> | delete <tenant> --record <path> | <resume-delete|prune-delete|reclaim-delete> <path> | recover-delete <tenant> <expected-tenant-id> | advance-delete-cutoff <tenant> <target-generation>";
const CONTROL_TENANT_ENV: &str = "FDB_TENANT_ADMIN_CONTROL_TENANT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
enum AdminToolError {
    #[error("{USAGE}")]
    Usage,
    #[error("tenant name must use lowercase ASCII letters, digits, and interior hyphens")]
    InvalidTenant,
    #[error("expected tenant ID must be a nonnegative integer")]
    InvalidTenantId,
    #[error("invalid FoundationDB configuration")]
    InvalidConfig,
    #[error("failed to connect to FoundationDB")]
    ConnectFailed,
    #[error("tenant ensure operation failed")]
    EnsureFailed,
    #[error("tenant metadata repair operation failed")]
    RepairFailed,
    #[error("tenant interrupted-delete recovery operation failed")]
    RecoverDeleteFailed,
    #[error("tenant delete operation failed")]
    DeleteFailed,
    #[error("tenant exists operation failed")]
    ExistsFailed,
    #[error("tenant ID lookup failed")]
    IdLookupFailed,
    #[error("tenant does not exist")]
    TenantMissing,
    #[error("could not verify the FoundationDB cluster identity")]
    ClusterIdentityUnavailable,
    #[error("delete record belongs to a different FoundationDB cluster")]
    ClusterMismatch,
    #[error("recorded delete failed; use resume-delete to reconcile its outcome")]
    RecordedDeleteFailed,
    #[error("delete reconciliation failed; the record remains pending")]
    ResumeFailed,
    #[error("could not write command output")]
    OutputFailed,
    #[error(transparent)]
    Record(#[from] DeleteRecordError),
    #[error(transparent)]
    Reservation(#[from] ReservationErrorReason),
    #[error("invalid reservation control tenant configuration")]
    InvalidControlTenant,
    #[error("this command requires FDB_TENANT_ADMIN_CONTROL_TENANT")]
    ControlTenantRequired,
    #[error("configured control tenant differs from the delete record")]
    ControlTenantMismatch,
    #[error("this delete record predates control-tenant coordination")]
    LegacyRecord,
    #[error("delete requires --record when a control tenant is configured")]
    RecordRequired,
}

enum Command {
    Ensure(FoundationDbTenantName),
    Repair(FoundationDbTenantName),
    RecoverDelete(FoundationDbTenantName, i64),
    Delete(FoundationDbTenantName),
    RecordedDelete(FoundationDbTenantName, PathBuf),
    ResumeDelete(PathBuf),
    PruneDelete(PathBuf),
    ReclaimDelete(PathBuf),
    TakeoverDelete(FoundationDbTenantName),
    DeleteCutoff(FoundationDbTenantName),
    AdvanceDeleteCutoff(FoundationDbTenantName, ReservationGeneration),
    Exists(FoundationDbTenantName),
    Id(FoundationDbTenantName),
}

fn parse_command(mut args: impl Iterator<Item = OsString>) -> Result<Command, AdminToolError> {
    let command = args.next().ok_or(AdminToolError::Usage)?;
    let operand = args.next().ok_or(AdminToolError::Usage)?;
    let parsed = if command == "resume-delete" {
        Command::ResumeDelete(PathBuf::from(operand))
    } else if command == "prune-delete" {
        Command::PruneDelete(PathBuf::from(operand))
    } else if command == "reclaim-delete" {
        Command::ReclaimDelete(PathBuf::from(operand))
    } else {
        let tenant = parse_tenant(
            operand
                .into_string()
                .map_err(|_| AdminToolError::InvalidTenant)?,
        )?;
        match command.to_str() {
            Some("ensure") => Command::Ensure(tenant),
            Some("repair") => Command::Repair(tenant),
            Some("recover-delete") => {
                let id = args
                    .next()
                    .ok_or(AdminToolError::Usage)?
                    .into_string()
                    .map_err(|_| AdminToolError::InvalidTenantId)?;
                Command::RecoverDelete(tenant, parse_expected_tenant_id(id)?)
            }
            Some("delete") => match args.next() {
                None => Command::Delete(tenant),
                Some(flag) if flag == "--record" => Command::RecordedDelete(
                    tenant,
                    PathBuf::from(args.next().ok_or(AdminToolError::Usage)?),
                ),
                Some(_) => return Err(AdminToolError::Usage),
            },
            Some("exists") => Command::Exists(tenant),
            Some("id") => Command::Id(tenant),
            Some("takeover-delete") => Command::TakeoverDelete(tenant),
            Some("delete-cutoff") => Command::DeleteCutoff(tenant),
            Some("advance-delete-cutoff") => {
                let target = args.next().ok_or(AdminToolError::Usage)?;
                let invalid = ReservationErrorReason::InvalidCutoffTarget;
                let target = target.to_str().ok_or(invalid)?;
                Command::AdvanceDeleteCutoff(
                    tenant,
                    delete_record::parse_generation(target).ok_or(invalid)?,
                )
            }
            _ => return Err(AdminToolError::Usage),
        }
    };
    if args.next().is_some() {
        return Err(AdminToolError::Usage);
    }
    Ok(parsed)
}

async fn run(command: Command) -> Result<(), AdminToolError> {
    let config = FdbConfig::from_env().map_err(|_| AdminToolError::InvalidConfig)?;
    // SAFETY: the one-shot CLI returns through main after all tenant handles
    // and this connector are dropped; it never calls process::exit.
    #[allow(unsafe_code)]
    let context =
        unsafe { FdbContext::connect(&config) }.map_err(|_| AdminToolError::ConnectFailed)?;
    match command {
        Command::Ensure(tenant) => admin::ensure_tenant(&context, tenant)
            .await
            .map_err(|_| AdminToolError::EnsureFailed),
        Command::Repair(tenant) => admin::repair_tenant_metadata(&context, tenant)
            .await
            .map_err(|_| AdminToolError::RepairFailed),
        Command::RecoverDelete(tenant, id) => {
            admin::recover_interrupted_delete(&context, tenant, id)
                .await
                .map_err(|_| AdminToolError::RecoverDeleteFailed)
        }
        Command::Delete(tenant) => {
            if configured_control_tenant()?.is_some() {
                return Err(AdminToolError::RecordRequired);
            }
            admin::delete_tenant(&context, tenant)
                .await
                .map_err(|_| AdminToolError::DeleteFailed)
        }
        Command::RecordedDelete(tenant, path) => recorded_delete(&context, tenant, &path).await,
        Command::ResumeDelete(path) => {
            let state = resume_delete(&context, &path).await?;
            write_output(state.as_str())
        }
        Command::PruneDelete(path) => {
            let state = prune_delete(&context, &path).await?;
            write_output(state.as_str())
        }
        Command::ReclaimDelete(path) => {
            reclaim_delete(&context, &path).await?;
            write_output("retired")
        }
        Command::TakeoverDelete(tenant) => {
            let control = required_control(&context).await?;
            let outcome = ReservationStore::new(&context, control)
                .takeover(tenant)
                .await?;
            write_output(record_state(outcome).as_str())
        }
        Command::DeleteCutoff(tenant) => {
            let control = required_control(&context).await?;
            let generation = ReservationStore::new(&context, control)
                .cutoff(tenant)
                .await?;
            write_output(generation.value())
        }
        Command::AdvanceDeleteCutoff(tenant, target) => {
            let control = required_control(&context).await?;
            let generation = ReservationStore::new(&context, control)
                .advance_cutoff(tenant, target)
                .await?;
            write_output(generation.value())
        }
        Command::Exists(tenant) => {
            let exists = admin::tenant_exists(&context, tenant)
                .await
                .map_err(|_| AdminToolError::ExistsFailed)?;
            write_output(exists)
        }
        Command::Id(tenant) => {
            let id = admin::tenant_id(&context, tenant)
                .await
                .map_err(|_| AdminToolError::IdLookupFailed)?
                .ok_or(AdminToolError::TenantMissing)?;
            write_output(id)
        }
    }
}

async fn recorded_delete(
    context: &FdbContext,
    tenant: FoundationDbTenantName,
    path: &Path,
) -> Result<(), AdminToolError> {
    let file = LockedDeleteRecord::open(path)?;
    let control = if let Some(control) = configured_control_tenant()? {
        if control == tenant {
            return Err(ReservationErrorReason::ControlTenantTarget.into());
        }
        Some(ReservationControlTenant::lookup(context, control).await?)
    } else {
        None
    };
    let record = persist_delete_intent(context, tenant, &file, control).await?;
    let outcome = if let Some(reservation) = record.reservation {
        ReservationStore::new(context, reservation.control)
            .delete(intent(record, reservation))
            .await?
    } else {
        admin::delete_recorded_tenant(context, tenant, record.original_id)
            .await
            .map_err(|_| AdminToolError::RecordedDeleteFailed)?;
        admin::TenantDeleteResolution::OriginalAbsent
    };
    file.complete(record, record_state(outcome))?;
    Ok(())
}

async fn persist_delete_intent(
    context: &FdbContext,
    tenant: FoundationDbTenantName,
    file: &LockedDeleteRecord,
    control: Option<ReservationControlTenant>,
) -> Result<DeleteRecord, AdminToolError> {
    let cluster = admin::tenant_admin_cluster_id(context)
        .await
        .map_err(|_| AdminToolError::ClusterIdentityUnavailable)?;
    let (original_id, reservation) = if let Some(control) = control {
        let captured = ReservationStore::new(context, control)
            .capture_intent(tenant)
            .await?
            .ok_or(AdminToolError::TenantMissing)?;
        (
            captured.original_id,
            Some(DeleteReservation {
                control,
                operation: captured.operation,
                generation: captured.generation,
            }),
        )
    } else {
        let id = admin::tenant_id(context, tenant)
            .await
            .map_err(|_| AdminToolError::IdLookupFailed)?
            .filter(|id| *id >= 0)
            .ok_or(AdminToolError::TenantMissing)?;
        (id, None)
    };
    let record = DeleteRecord {
        cluster,
        tenant,
        original_id,
        state: DeleteRecordState::Pending,
        reservation,
    };
    // Keep the lock from durable creation through the final database result.
    // A resumed worker can only race transactions already submitted by this one.
    file.create(record)?;
    Ok(record)
}

async fn resume_delete(
    context: &FdbContext,
    path: &Path,
) -> Result<DeleteRecordState, AdminToolError> {
    let file = LockedDeleteRecord::open(path)?;
    let record = load_verified_record(context, &file).await?;
    if record.state != DeleteRecordState::Pending {
        // A previous process may have renamed the completed record and then
        // failed before synchronizing its directory entry.
        file.sync_directory()?;
        return Ok(record.state);
    }
    let configured_control = configured_control_tenant()?;
    let outcome = if let Some(reservation) = record.reservation {
        check_configured_control(configured_control, reservation.control)?;
        ReservationStore::new(context, reservation.control)
            .resume(intent(record, reservation))
            .await?
    } else {
        if configured_control.is_some() {
            return Err(AdminToolError::LegacyRecord);
        }
        admin::reconcile_recorded_delete(context, record.tenant, record.original_id)
            .await
            .map_err(|_| AdminToolError::ResumeFailed)?
    };
    let state = record_state(outcome);
    file.complete(record, state)?;
    Ok(state)
}

async fn prune_delete(
    context: &FdbContext,
    path: &Path,
) -> Result<DeleteRecordState, AdminToolError> {
    let file = LockedDeleteRecord::open(path)?;
    let record = load_verified_record(context, &file).await?;
    let reservation = record.reservation.ok_or(AdminToolError::LegacyRecord)?;
    check_configured_control(configured_control_tenant()?, reservation.control)?;
    let outcome = ReservationStore::new(context, reservation.control)
        .prune_completed(intent(record, reservation))
        .await?;
    Ok(record_state(outcome))
}

async fn reclaim_delete(context: &FdbContext, path: &Path) -> Result<(), AdminToolError> {
    let file = LockedDeleteRecord::open(path)?;
    let record = load_verified_record(context, &file).await?;
    let reservation = record.reservation.ok_or(AdminToolError::LegacyRecord)?;
    check_configured_control(configured_control_tenant()?, reservation.control)?;
    ReservationStore::new(context, reservation.control)
        .reclaim_tombstone(intent(record, reservation))
        .await?;
    Ok(())
}

async fn load_verified_record(
    context: &FdbContext,
    file: &LockedDeleteRecord,
) -> Result<DeleteRecord, AdminToolError> {
    let record = file.load()?;
    let cluster = admin::tenant_admin_cluster_id(context)
        .await
        .map_err(|_| AdminToolError::ClusterIdentityUnavailable)?;
    if cluster != record.cluster {
        return Err(AdminToolError::ClusterMismatch);
    }
    Ok(record)
}

fn check_configured_control(
    configured: Option<FoundationDbTenantName>,
    recorded: ReservationControlTenant,
) -> Result<(), AdminToolError> {
    if configured.is_some_and(|name| name != recorded.name()) {
        return Err(AdminToolError::ControlTenantMismatch);
    }
    Ok(())
}

fn configured_control_tenant() -> Result<Option<FoundationDbTenantName>, AdminToolError> {
    match env::var(CONTROL_TENANT_ENV) {
        Ok(value) => FoundationDbTenantName::new(&value)
            .map(Some)
            .map_err(|_| AdminToolError::InvalidControlTenant),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(env::VarError::NotUnicode(_)) => Err(AdminToolError::InvalidControlTenant),
    }
}

async fn required_control(
    context: &FdbContext,
) -> Result<ReservationControlTenant, AdminToolError> {
    let control = configured_control_tenant()?.ok_or(AdminToolError::ControlTenantRequired)?;
    Ok(ReservationControlTenant::lookup(context, control).await?)
}

fn intent(record: DeleteRecord, reservation: DeleteReservation) -> ReservationIntent {
    ReservationIntent {
        tenant: record.tenant,
        original_id: record.original_id,
        operation: reservation.operation,
        generation: reservation.generation,
    }
}

const fn record_state(outcome: admin::TenantDeleteResolution) -> DeleteRecordState {
    match outcome {
        admin::TenantDeleteResolution::OriginalAbsent => DeleteRecordState::OriginalAbsent,
        admin::TenantDeleteResolution::MetadataSafe => DeleteRecordState::MetadataSafe,
    }
}

fn write_output(value: impl std::fmt::Display) -> Result<(), AdminToolError> {
    writeln!(io::stdout().lock(), "{value}").map_err(|_| AdminToolError::OutputFailed)
}

fn parse_tenant(value: String) -> Result<FoundationDbTenantName, AdminToolError> {
    FoundationDbTenantName::new(value.as_str()).map_err(|_| AdminToolError::InvalidTenant)
}

fn parse_expected_tenant_id(value: String) -> Result<i64, AdminToolError> {
    value
        .parse::<i64>()
        .ok()
        .filter(|id| *id >= 0)
        .ok_or(AdminToolError::InvalidTenantId)
}

#[tokio::main]
async fn main() -> ExitCode {
    let result = match parse_command(env::args_os().skip(1)) {
        Ok(command) => run(command).await,
        Err(error) => Err(error),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(io::stderr().lock(), "{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "fdb-tenant-admin_tests.rs"]
mod tests;
