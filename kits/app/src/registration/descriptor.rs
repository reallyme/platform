// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use super::AppDependency;
use crate::metadata::AppMetadata;

/// Host-neutral app registration descriptor.
///
/// This type describes what an app is and what other apps it requires. It does
/// not register routes, bind listeners, or start work; concrete hosts decide
/// how to adapt the descriptor into their runtime model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppRegistration {
    metadata: AppMetadata,
    dependencies: Vec<AppDependency>,
}

impl AppRegistration {
    /// Constructs an app registration descriptor without dependencies.
    pub const fn new(metadata: AppMetadata) -> Self {
        Self {
            metadata,
            dependencies: Vec::new(),
        }
    }

    /// Returns app metadata.
    pub const fn metadata(&self) -> &AppMetadata {
        &self.metadata
    }

    /// Adds a required app dependency.
    pub fn with_dependency(mut self, dependency: AppDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Returns required app dependencies in declared order.
    pub fn dependencies(&self) -> &[AppDependency] {
        self.dependencies.as_slice()
    }
}
