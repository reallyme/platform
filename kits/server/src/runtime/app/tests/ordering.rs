// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::{Arc, Mutex};

use axum::Router;

use super::super::{AppHttpMountPath, AppName, RuntimeApp, RuntimeAppDependency, collect_apps};
use super::fixtures::recording_startup_check;

#[tokio::test]
async fn app_starts_after_its_dependency() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let dependency = RuntimeApp::new(
        AppName::new("dependency").expect("valid fixture app name"),
        Router::new(),
    )
    .with_startup_check(recording_startup_check(
        "dependency-check",
        "dependency",
        &observed,
    ));
    let dependency_handle = dependency.handle();
    let dependent = RuntimeApp::new(
        AppName::new("dependent").expect("valid fixture app name"),
        Router::new(),
    )
    .with_http_mount(AppHttpMountPath::new("/dependent").expect("valid fixture mount"))
    .with_dependency(RuntimeAppDependency::from_handle(&dependency_handle))
    .with_startup_check(recording_startup_check(
        "dependent-check",
        "dependent",
        &observed,
    ));

    let parts = collect_apps(vec![dependent, dependency]).expect("valid app dependency graph");

    for check in parts.startup_checks {
        check.run().await.expect("startup check should succeed");
    }

    assert_eq!(
        observed
            .lock()
            .expect("test mutex should not be poisoned")
            .as_slice(),
        ["dependency", "dependent"]
    );
}

#[test]
fn independent_apps_start_deterministically_in_input_order() {
    let first = RuntimeApp::new(
        AppName::new("first").expect("valid fixture app name"),
        Router::new(),
    );
    let second = RuntimeApp::new(
        AppName::new("second").expect("valid fixture app name"),
        Router::new(),
    )
    .with_http_mount(AppHttpMountPath::new("/second").expect("valid fixture mount"));

    let parts = collect_apps(vec![first, second]).expect("valid independent app graph");

    assert!(parts.startup_checks.is_empty());
    assert_eq!(parts.cleanup_hooks.len(), 0);
}

#[test]
fn one_app_collection_still_works() {
    let app = RuntimeApp::new(
        AppName::new("single").expect("valid fixture app name"),
        Router::new(),
    );

    assert!(collect_apps(vec![app]).is_ok());
}
