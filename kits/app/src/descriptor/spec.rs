// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppDescriptor;
use crate::AppKitError;
use crate::auth::{AppCapability, AppCapabilityName, AppPermission, AppPermissionName};
use crate::lifecycle::{
    AppBackgroundTaskDescriptor, AppCleanupHookDescriptor, AppLifecycleName,
    AppStartupCheckDescriptor,
};
use crate::metadata::{AppMetadata, AppName, AppVersion};
use crate::metrics::AppMetricNamespace;
use crate::ports::AppAdapterKind;

/// Declarative app descriptor spec.
///
/// Apps should prefer this compact spec over hand-assembling descriptors. It
/// keeps app descriptors consistent while still requiring each app crate to
/// provide its own name and crate version explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppDescriptorSpec {
    /// App name.
    pub app_name: &'static str,
    /// App crate version.
    pub app_version: &'static str,
    /// App metric namespace.
    pub metric_namespace: &'static str,
    /// App permission names.
    pub permissions: &'static [&'static str],
    /// App capability names.
    pub capabilities: &'static [&'static str],
    /// Host/transport adapter declarations.
    pub adapters: &'static [AppAdapterKind],
    /// Startup check names.
    pub startup_checks: &'static [&'static str],
    /// Background task names.
    pub background_tasks: &'static [&'static str],
    /// Cleanup hook names.
    pub cleanup_hooks: &'static [&'static str],
}

impl AppDescriptorSpec {
    /// Builds validated app metadata from the spec.
    pub fn metadata(self) -> Result<AppMetadata, AppKitError> {
        Ok(AppMetadata::new(
            AppName::new(self.app_name)?,
            AppVersion::new(self.app_version)?,
        ))
    }

    /// Builds a validated app descriptor.
    pub fn build(self) -> Result<AppDescriptor, AppKitError> {
        let mut descriptor = AppDescriptor::new(
            self.metadata()?,
            AppMetricNamespace::new(self.metric_namespace)?,
        );

        for (index, permission) in self.permissions.iter().enumerate() {
            if self.permissions[..index].contains(permission) {
                continue;
            }
            descriptor = descriptor
                .with_permission(AppPermission::new(AppPermissionName::new(*permission)?));
        }

        for (index, capability) in self.capabilities.iter().enumerate() {
            if self.capabilities[..index].contains(capability) {
                continue;
            }
            descriptor = descriptor
                .with_capability(AppCapability::new(AppCapabilityName::new(*capability)?));
        }

        for (index, adapter) in self.adapters.iter().enumerate() {
            if self.adapters[..index].contains(adapter) {
                continue;
            }
            descriptor = descriptor.with_adapter(*adapter);
        }

        for (index, startup_check) in self.startup_checks.iter().enumerate() {
            if self.startup_checks[..index].contains(startup_check) {
                continue;
            }
            descriptor = descriptor.with_startup_check(AppStartupCheckDescriptor::new(
                AppLifecycleName::new(*startup_check)?,
            ));
        }

        for (index, background_task) in self.background_tasks.iter().enumerate() {
            if self.background_tasks[..index].contains(background_task) {
                continue;
            }
            descriptor = descriptor.with_background_task(AppBackgroundTaskDescriptor::new(
                AppLifecycleName::new(*background_task)?,
            ));
        }

        for (index, cleanup_hook) in self.cleanup_hooks.iter().enumerate() {
            if self.cleanup_hooks[..index].contains(cleanup_hook) {
                continue;
            }
            descriptor = descriptor.with_cleanup_hook(AppCleanupHookDescriptor::new(
                AppLifecycleName::new(*cleanup_hook)?,
            ));
        }

        Ok(descriptor)
    }
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
