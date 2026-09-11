// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Standalone tenant operations for FoundationDB operator workflows.
//!
//! Invocations:
//! - `ensure <tenant-name>`
//! - `delete <tenant-name>`
//! - `exists <tenant-name>`

use std::env;
use std::process::ExitCode;

use reallyme_foundationdb_kit::FoundationDbTenantName;
use reallyme_foundationdb_kit::{FdbConfig, FdbContext, fdb::tenant::admin};
use thiserror::Error;

const USAGE: &str = "usage: fdb-tenant-admin <ensure|delete|exists> <tenant>";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
enum AdminToolError {
    #[error("{USAGE}")]
    Usage,
    #[error("tenant name must use lowercase ASCII letters, digits, and interior hyphens")]
    InvalidTenant,
    #[error("invalid FoundationDB configuration")]
    InvalidConfig,
    #[error("failed to connect to FoundationDB")]
    ConnectFailed,
    #[error("tenant ensure operation failed")]
    EnsureFailed,
    #[error("tenant delete operation failed")]
    DeleteFailed,
    #[error("tenant exists operation failed")]
    ExistsFailed,
}

async fn run() -> Result<(), AdminToolError> {
    let mut args = env::args().skip(1);
    let command = args.next().ok_or(AdminToolError::Usage)?;
    let tenant = args.next().ok_or(AdminToolError::Usage)?;

    if args.next().is_some() {
        return Err(AdminToolError::Usage);
    }

    let tenant = parse_tenant(tenant)?;

    let config = FdbConfig::from_env().map_err(|_| AdminToolError::InvalidConfig)?;
    let context = FdbContext::connect(&config).map_err(|_| AdminToolError::ConnectFailed)?;
    let database = context.database_for_admin();

    match command.as_str() {
        "ensure" => admin::ensure_tenant(database, tenant)
            .await
            .map_err(|_| AdminToolError::EnsureFailed),
        "delete" => admin::delete_tenant(database, tenant)
            .await
            .map_err(|_| AdminToolError::DeleteFailed),
        "exists" => {
            let exists = admin::tenant_exists(database, tenant)
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
mod tests {
    use reallyme_foundationdb_kit::FoundationDbTenantName;

    use super::{AdminToolError, parse_tenant};

    #[test]
    fn parses_valid_tenant() {
        let expected = FoundationDbTenantName::new("catalog");
        assert!(expected.is_ok());
        let Ok(expected) = expected else {
            return;
        };
        assert_eq!(parse_tenant(String::from("catalog")), Ok(expected));
    }

    #[test]
    fn accepts_deployment_owned_tenant() {
        assert!(parse_tenant(String::from("production-search")).is_ok());
    }

    #[test]
    fn rejects_invalid_dynamic_tenant() {
        assert_eq!(
            parse_tenant(String::from("Customer_Supplied")),
            Err(AdminToolError::InvalidTenant)
        );
    }
}
