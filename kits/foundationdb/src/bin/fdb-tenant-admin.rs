// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Standalone tenant operations for FoundationDB operator workflows.
//!
//! Invocations:
//! - `ensure <tenant-name>`
//! - `repair <tenant-name>`
//! - `recover-delete <tenant-name> <expected-tenant-id>`
//! - `delete <tenant-name>`
//! - `exists <tenant-name>`

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

use std::env;
use std::process::ExitCode;

use reallyme_foundationdb_kit::FoundationDbTenantName;
use reallyme_foundationdb_kit::{FdbConfig, FdbContext, fdb::tenant::admin};
use thiserror::Error;

const USAGE: &str = "usage: fdb-tenant-admin <ensure|repair|delete|exists> <tenant> | recover-delete <tenant> <expected-tenant-id>";

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
}

async fn run() -> Result<(), AdminToolError> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(AdminToolError::Usage)?;
    let tenant = args.next().ok_or(AdminToolError::Usage)?;
    let expected_tenant_id = if command == "recover-delete" {
        Some(parse_expected_tenant_id(
            args.next().ok_or(AdminToolError::Usage)?,
        )?)
    } else {
        None
    };
    if args.next().is_some() {
        return Err(AdminToolError::Usage);
    }

    let tenant = parse_tenant(tenant)?;

    let config = FdbConfig::from_env().map_err(|_| AdminToolError::InvalidConfig)?;
    // SAFETY: the one-shot CLI returns through main after all tenant handles
    // and this connector are dropped; it never calls process::exit.
    #[allow(unsafe_code)]
    let context =
        unsafe { FdbContext::connect(&config) }.map_err(|_| AdminToolError::ConnectFailed)?;
    match command.as_str() {
        "ensure" => admin::ensure_tenant(&context, tenant)
            .await
            .map_err(|_| AdminToolError::EnsureFailed),
        "repair" => admin::repair_tenant_metadata(&context, tenant)
            .await
            .map_err(|_| AdminToolError::RepairFailed),
        "recover-delete" => admin::recover_interrupted_delete(
            &context,
            tenant,
            expected_tenant_id.ok_or(AdminToolError::Usage)?,
        )
        .await
        .map_err(|_| AdminToolError::RecoverDeleteFailed),
        "delete" => admin::delete_tenant(&context, tenant)
            .await
            .map_err(|_| AdminToolError::DeleteFailed),
        "exists" => {
            let exists = admin::tenant_exists(&context, tenant)
                .await
                .map_err(|_| AdminToolError::ExistsFailed)?;
            // Deliberate exception: this one-shot operator CLI uses stdout as its
            // interface contract for machine/human scripting (`true` / `false`).
            if exists {
                println!("true");
            } else {
                println!("false");
            }
            Ok(())
        }
        _ => Err(AdminToolError::Usage),
    }
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
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            // Deliberate exception: stderr is the operator-facing error channel
            // for this CLI, not an application runtime log surface.
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "fdb-tenant-admin_tests.rs"]
mod tests;
