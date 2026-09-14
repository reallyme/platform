// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use crate::metadata::AppName;

/// Required dependency on another logical app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDependency {
    app_name: AppName,
}

impl AppDependency {
    /// Constructs an app dependency.
    pub const fn new(app_name: AppName) -> Self {
        Self { app_name }
    }

    /// Returns the dependency app name.
    pub const fn app_name(&self) -> &AppName {
        &self.app_name
    }
}
