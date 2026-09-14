// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppDescriptorSpec;
use crate::ports::AppAdapterKind;

#[test]
fn descriptor_spec_builds_standard_descriptor() {
    let descriptor = AppDescriptorSpec {
        app_name: "template",
        app_version: "1.2.3",
        metric_namespace: "template",
        permissions: &["template.status"],
        capabilities: &["http", "server"],
        adapters: &[AppAdapterKind::Http, AppAdapterKind::Server],
        startup_checks: &["startup-check"],
        background_tasks: &["background-task"],
        cleanup_hooks: &["cleanup-hook"],
    }
    .build()
    .expect("valid descriptor spec fixture");

    assert_eq!(descriptor.metadata().name().as_str(), "template");
    assert_eq!(descriptor.metadata().version().as_str(), "1.2.3");
    assert_eq!(descriptor.permissions().len(), 1);
    assert_eq!(descriptor.cleanup_hooks().len(), 1);
}

#[test]
fn descriptor_spec_deduplicates_duplicate_entries() {
    let descriptor = AppDescriptorSpec {
        app_name: "template",
        app_version: "1.2.3",
        metric_namespace: "template",
        permissions: &["template.status", "template.status", "template.publish"],
        capabilities: &["http", "http", "server"],
        adapters: &[
            AppAdapterKind::Http,
            AppAdapterKind::Http,
            AppAdapterKind::Server,
        ],
        startup_checks: &["startup-check", "startup-check"],
        background_tasks: &["background-task", "background-task"],
        cleanup_hooks: &["cleanup-hook", "cleanup-hook"],
    }
    .build()
    .expect("descriptor spec fixture with duplicates should build");

    assert_eq!(descriptor.permissions().len(), 2);
    assert_eq!(descriptor.capabilities().len(), 2);
    assert_eq!(descriptor.adapters().len(), 2);
    assert_eq!(descriptor.startup_checks().len(), 1);
    assert_eq!(descriptor.background_tasks().len(), 1);
    assert_eq!(descriptor.cleanup_hooks().len(), 1);
}
