// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! App metadata primitives.

mod app_metadata;
mod app_name;
mod app_version;

pub use app_metadata::AppMetadata;
pub use app_name::AppName;
pub use app_version::AppVersion;

pub(crate) use app_name::validate_dns_label;
