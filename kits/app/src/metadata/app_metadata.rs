// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use super::{AppName, AppVersion};

/// Static metadata describing a logical app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppMetadata {
    name: AppName,
    version: AppVersion,
}

impl AppMetadata {
    /// Constructs app metadata.
    pub const fn new(name: AppName, version: AppVersion) -> Self {
        Self { name, version }
    }

    /// Returns the app name.
    pub const fn name(&self) -> &AppName {
        &self.name
    }

    /// Returns the app version.
    pub const fn version(&self) -> &AppVersion {
        &self.version
    }
}
