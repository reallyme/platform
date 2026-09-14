// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
