// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app authorization contract primitives.

mod capability;
mod permission;

pub use capability::{AppCapability, AppCapabilityName};
pub use permission::{AppPermission, AppPermissionName};

pub(crate) use permission::validate_symbolic_name;
