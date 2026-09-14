// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runtime-agnostic tenant provisioning verification helpers.

use crate::{FdbContext, FdbHealthReport, FdbResult, FoundationDbTenantName};

/// Runs the complete FoundationDB startup readiness gate.
///
/// Cluster reachability is verified before tenant metadata so an unavailable
/// cluster is reported as a connector-health failure rather than being
/// conflated with tenant provisioning.
pub async fn verify_ready(
    context: &FdbContext,
    tenants: &[FoundationDbTenantName],
) -> FdbResult<FdbHealthReport> {
    let health = context.health_report().await?;
    verify_tenants_provisioned(context, tenants).await?;
    Ok(health)
}

/// Verifies all provided tenants are provisioned and readable.
///
/// This helper is runtime-agnostic by design so server composition layers can
/// wrap it in their own readiness/startup abstractions without inverting
/// dependency direction back into infrastructure kits.
pub async fn verify_tenants_provisioned(
    context: &FdbContext,
    tenants: &[FoundationDbTenantName],
) -> FdbResult<()> {
    for tenant in tenants {
        let tenant_handle = context.open_tenant(*tenant).await?;
        drop(tenant_handle);
    }

    Ok(())
}
