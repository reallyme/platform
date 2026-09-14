// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::{Deserialize, Serialize};

use crate::auth::{AppCapability, AppPermission};
use crate::config::AppConfigSource;
use crate::lifecycle::{
    AppBackgroundTaskDescriptor, AppCleanupHookDescriptor, AppStartupCheckDescriptor,
};
use crate::metadata::AppMetadata;
use crate::metrics::AppMetricNamespace;
use crate::ports::AppAdapterKind;
use crate::registration::AppDependency;

/// Host-neutral descriptor for a ReallyMe app.
///
/// This descriptor is the standard app-kit summary of what an app is and what
/// it exposes. It intentionally does not contain host runtime objects such as
/// Axum routers, tonic services, listener sockets, shutdown tokens, or task
/// handles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppDescriptor {
    metadata: AppMetadata,
    config_source: Option<AppConfigSource>,
    metric_namespace: AppMetricNamespace,
    dependencies: Vec<AppDependency>,
    permissions: Vec<AppPermission>,
    capabilities: Vec<AppCapability>,
    adapters: Vec<AppAdapterKind>,
    startup_checks: Vec<AppStartupCheckDescriptor>,
    background_tasks: Vec<AppBackgroundTaskDescriptor>,
    cleanup_hooks: Vec<AppCleanupHookDescriptor>,
}

impl AppDescriptor {
    /// Constructs an app descriptor with no optional declarations.
    pub const fn new(metadata: AppMetadata, metric_namespace: AppMetricNamespace) -> Self {
        Self {
            metadata,
            config_source: None,
            metric_namespace,
            dependencies: Vec::new(),
            permissions: Vec::new(),
            capabilities: Vec::new(),
            adapters: Vec::new(),
            startup_checks: Vec::new(),
            background_tasks: Vec::new(),
            cleanup_hooks: Vec::new(),
        }
    }

    /// Returns app metadata.
    pub const fn metadata(&self) -> &AppMetadata {
        &self.metadata
    }

    /// Returns the optional safe config source descriptor.
    pub const fn config_source(&self) -> Option<&AppConfigSource> {
        self.config_source.as_ref()
    }

    /// Returns the app metric namespace.
    pub const fn metric_namespace(&self) -> &AppMetricNamespace {
        &self.metric_namespace
    }

    /// Returns declared app dependencies.
    pub fn dependencies(&self) -> &[AppDependency] {
        self.dependencies.as_slice()
    }

    /// Returns declared app permissions.
    pub fn permissions(&self) -> &[AppPermission] {
        self.permissions.as_slice()
    }

    /// Returns declared app capabilities.
    pub fn capabilities(&self) -> &[AppCapability] {
        self.capabilities.as_slice()
    }

    /// Returns declared adapter kinds.
    pub fn adapters(&self) -> &[AppAdapterKind] {
        self.adapters.as_slice()
    }

    /// Returns declared startup check descriptors.
    pub fn startup_checks(&self) -> &[AppStartupCheckDescriptor] {
        self.startup_checks.as_slice()
    }

    /// Returns declared background task descriptors.
    pub fn background_tasks(&self) -> &[AppBackgroundTaskDescriptor] {
        self.background_tasks.as_slice()
    }

    /// Returns declared cleanup hook descriptors.
    pub fn cleanup_hooks(&self) -> &[AppCleanupHookDescriptor] {
        self.cleanup_hooks.as_slice()
    }

    /// Sets the safe app config source descriptor.
    pub fn with_config_source(mut self, value: AppConfigSource) -> Self {
        self.config_source = Some(value);
        self
    }

    /// Adds a required app dependency.
    pub fn with_dependency(mut self, value: AppDependency) -> Self {
        Self::push_if_absent(&mut self.dependencies, value);
        self
    }

    /// Adds an app permission declaration.
    pub fn with_permission(mut self, value: AppPermission) -> Self {
        Self::push_if_absent(&mut self.permissions, value);
        self
    }

    /// Adds an app capability declaration.
    pub fn with_capability(mut self, value: AppCapability) -> Self {
        Self::push_if_absent(&mut self.capabilities, value);
        self
    }

    /// Adds an adapter kind declaration.
    pub fn with_adapter(mut self, value: AppAdapterKind) -> Self {
        Self::push_if_absent(&mut self.adapters, value);
        self
    }

    /// Adds a startup check descriptor.
    pub fn with_startup_check(mut self, value: AppStartupCheckDescriptor) -> Self {
        Self::push_if_absent(&mut self.startup_checks, value);
        self
    }

    /// Adds a background task descriptor.
    pub fn with_background_task(mut self, value: AppBackgroundTaskDescriptor) -> Self {
        Self::push_if_absent(&mut self.background_tasks, value);
        self
    }

    /// Adds a cleanup hook descriptor.
    pub fn with_cleanup_hook(mut self, value: AppCleanupHookDescriptor) -> Self {
        Self::push_if_absent(&mut self.cleanup_hooks, value);
        self
    }

    fn push_if_absent<T: PartialEq>(values: &mut Vec<T>, value: T) {
        if !values.contains(&value) {
            values.push(value);
        }
    }
}

#[cfg(test)]
#[path = "app_descriptor_tests.rs"]
mod tests;
