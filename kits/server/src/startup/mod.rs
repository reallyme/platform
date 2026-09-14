// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Startup-time server-process and task identity validation.

mod banner;
mod error;
mod region;
mod server_name;
mod task_name;

pub use banner::{StartupBanner, should_emit_startup_banner, write_startup_banner};
pub use error::StartupError;
pub use region::DeploymentRegion;
pub use server_name::ServerName;
pub use task_name::TaskName;

#[cfg(test)]
mod tests;
