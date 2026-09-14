// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppDescriptor;
use crate::{
    AppAdapterKind, AppBackgroundTaskDescriptor, AppCapability, AppCapabilityName,
    AppCleanupHookDescriptor, AppLifecycleName, AppMetadata, AppMetricNamespace, AppName,
    AppPermission, AppPermissionName, AppStartupCheckDescriptor, AppVersion,
};

#[test]
fn app_descriptor_captures_host_neutral_app_shape() {
    let descriptor = AppDescriptor::new(
        AppMetadata::new(
            AppName::new("template").expect("valid app name fixture"),
            AppVersion::new("0.1.0").expect("valid app version fixture"),
        ),
        AppMetricNamespace::new("template").expect("valid metric namespace fixture"),
    )
    .with_capability(AppCapability::new(
        AppCapabilityName::new("http").expect("valid capability name fixture"),
    ))
    .with_adapter(AppAdapterKind::Http);

    assert_eq!(descriptor.metadata().name().as_str(), "template");
    assert_eq!(descriptor.capabilities().len(), 1);
    assert_eq!(descriptor.adapters(), &[AppAdapterKind::Http]);
}

#[test]
fn app_descriptor_builder_deduplicates_repeated_entries() {
    let descriptor = AppDescriptor::new(
        AppMetadata::new(
            AppName::new("template").expect("valid app name fixture"),
            AppVersion::new("0.1.0").expect("valid app version fixture"),
        ),
        AppMetricNamespace::new("template").expect("valid metric namespace fixture"),
    )
    .with_permission(AppPermission::new(
        AppPermissionName::new("template.status").expect("valid permission name fixture"),
    ))
    .with_permission(AppPermission::new(
        AppPermissionName::new("template.status").expect("valid permission name fixture"),
    ))
    .with_capability(AppCapability::new(
        AppCapabilityName::new("http").expect("valid capability name fixture"),
    ))
    .with_capability(AppCapability::new(
        AppCapabilityName::new("http").expect("valid capability name fixture"),
    ))
    .with_adapter(AppAdapterKind::Http)
    .with_adapter(AppAdapterKind::Http)
    .with_startup_check(AppStartupCheckDescriptor::new(
        AppLifecycleName::new("startup-check").expect("valid lifecycle name fixture"),
    ))
    .with_startup_check(AppStartupCheckDescriptor::new(
        AppLifecycleName::new("startup-check").expect("valid lifecycle name fixture"),
    ))
    .with_background_task(AppBackgroundTaskDescriptor::new(
        AppLifecycleName::new("background-task").expect("valid lifecycle name fixture"),
    ))
    .with_background_task(AppBackgroundTaskDescriptor::new(
        AppLifecycleName::new("background-task").expect("valid lifecycle name fixture"),
    ))
    .with_cleanup_hook(AppCleanupHookDescriptor::new(
        AppLifecycleName::new("cleanup-hook").expect("valid lifecycle name fixture"),
    ))
    .with_cleanup_hook(AppCleanupHookDescriptor::new(
        AppLifecycleName::new("cleanup-hook").expect("valid lifecycle name fixture"),
    ));

    assert_eq!(descriptor.permissions().len(), 1);
    assert_eq!(descriptor.capabilities().len(), 1);
    assert_eq!(descriptor.adapters(), &[AppAdapterKind::Http]);
    assert_eq!(descriptor.startup_checks().len(), 1);
    assert_eq!(descriptor.background_tasks().len(), 1);
    assert_eq!(descriptor.cleanup_hooks().len(), 1);
}
