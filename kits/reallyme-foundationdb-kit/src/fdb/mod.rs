// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! FoundationDB client boundary and low-level database primitives.

pub mod config;
pub mod connector;
pub mod error;
pub mod range;
pub mod startup;
pub mod tenant;
pub mod tenant_name;
pub mod transaction;
pub mod tuple;

pub use connector::FdbContext;
pub use tenant::TenantHandle;
pub use tenant_name::FoundationDbTenantName;
