// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::{
    AppAdapterKind, AppDescriptor, AppDescriptorSpec, AppKitError, AppMetadata, AppName,
};

/// Stable app name used by host compositions.
pub const EXAMPLE_APP_NAME: &str = "example-app";

/// Returns the validated typed app name.
#[allow(dead_code)]
pub fn validated_example_app_name() -> Result<AppName, AppKitError> {
    AppName::new(EXAMPLE_APP_NAME)
}

/// Stable lifecycle name for the example app's startup readiness gate.
pub const EXAMPLE_STARTUP_CHECK_NAME: &str = "startup-check";
/// Stable lifecycle name for the example app's background task.
pub const EXAMPLE_BACKGROUND_TASK_NAME: &str = "background-task";
/// Stable lifecycle name for the example app's cleanup hook.
pub const EXAMPLE_CLEANUP_HOOK_NAME: &str = "cleanup-hook";

const EXAMPLE_APP_DESCRIPTOR_SPEC: AppDescriptorSpec = AppDescriptorSpec {
    app_name: EXAMPLE_APP_NAME,
    app_version: env!("CARGO_PKG_VERSION"),
    metric_namespace: "reallyme_example",
    permissions: &["reallyme-example.hello"],
    capabilities: &["connect_rpc", "http", "server", "worker"],
    adapters: &[
        AppAdapterKind::ConnectRpc,
        AppAdapterKind::Http,
        AppAdapterKind::Server,
        AppAdapterKind::Worker,
    ],
    startup_checks: &[EXAMPLE_STARTUP_CHECK_NAME],
    background_tasks: &[EXAMPLE_BACKGROUND_TASK_NAME],
    cleanup_hooks: &[EXAMPLE_CLEANUP_HOOK_NAME],
};

/// Returns host-neutral example app metadata.
pub fn example_app_metadata() -> Result<AppMetadata, AppKitError> {
    EXAMPLE_APP_DESCRIPTOR_SPEC.metadata()
}

/// Returns the host-neutral example app descriptor.
pub fn example_app_descriptor() -> Result<AppDescriptor, AppKitError> {
    EXAMPLE_APP_DESCRIPTOR_SPEC.build()
}

#[cfg(test)]
mod tests {
    use reallyme_app_kit::AppAdapterKind;

    use super::{EXAMPLE_APP_NAME, example_app_descriptor, validated_example_app_name};

    #[test]
    fn descriptor_declares_connect_http_server_and_worker_adapters() {
        let descriptor = example_app_descriptor().expect("valid example descriptor fixture");

        assert_eq!(descriptor.metadata().name().as_str(), EXAMPLE_APP_NAME);
        assert_eq!(
            descriptor.adapters(),
            &[
                AppAdapterKind::ConnectRpc,
                AppAdapterKind::Http,
                AppAdapterKind::Server,
                AppAdapterKind::Worker,
            ],
        );
        assert_eq!(descriptor.startup_checks().len(), 1);
        assert_eq!(descriptor.background_tasks().len(), 1);
        assert_eq!(descriptor.cleanup_hooks().len(), 1);
        assert_eq!(
            validated_example_app_name()
                .expect("validated example app name fixture")
                .as_str(),
            EXAMPLE_APP_NAME,
        );
    }
}
