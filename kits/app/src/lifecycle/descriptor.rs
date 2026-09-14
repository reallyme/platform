// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use super::AppLifecycleName;

/// Host-neutral descriptor for an app startup check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppStartupCheckDescriptor {
    name: AppLifecycleName,
}

impl AppStartupCheckDescriptor {
    /// Constructs a startup check descriptor.
    pub const fn new(name: AppLifecycleName) -> Self {
        Self { name }
    }

    /// Returns the startup check name.
    pub const fn name(&self) -> &AppLifecycleName {
        &self.name
    }
}

/// Host-neutral descriptor for an app background task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppBackgroundTaskDescriptor {
    name: AppLifecycleName,
}

impl AppBackgroundTaskDescriptor {
    /// Constructs a background task descriptor.
    pub const fn new(name: AppLifecycleName) -> Self {
        Self { name }
    }

    /// Returns the background task name.
    pub const fn name(&self) -> &AppLifecycleName {
        &self.name
    }
}

/// Host-neutral descriptor for an app cleanup hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppCleanupHookDescriptor {
    name: AppLifecycleName,
}

impl AppCleanupHookDescriptor {
    /// Constructs a cleanup hook descriptor.
    pub const fn new(name: AppLifecycleName) -> Self {
        Self { name }
    }

    /// Returns the cleanup hook name.
    pub const fn name(&self) -> &AppLifecycleName {
        &self.name
    }
}
