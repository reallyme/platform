// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;

use super::super::{AppHttpMountPath, AppName, RuntimeApp, RuntimeAppDependency, collect_apps};
use crate::runtime::{RuntimeAppCompositionErrorReason, ServerRuntimeError};

#[test]
fn unknown_dependency_fails_closed() {
    let app = RuntimeApp::new(
        AppName::new("api").expect("valid fixture app name"),
        Router::new(),
    )
    .with_dependency(RuntimeAppDependency::from_app_name(
        AppName::new("missing").expect("valid fixture app name"),
    ));

    assert!(matches!(
        collect_apps(vec![app]),
        Err(ServerRuntimeError::AppComposition {
            reason: RuntimeAppCompositionErrorReason::UnknownDependency,
        })
    ));
}

#[test]
fn dependency_cycle_fails_closed() {
    let first_name = AppName::new("first").expect("valid fixture app name");
    let second_name = AppName::new("second").expect("valid fixture app name");
    let first = RuntimeApp::new(first_name.clone(), Router::new())
        .with_dependency(RuntimeAppDependency::from_app_name(second_name.clone()));
    let second = RuntimeApp::new(second_name, Router::new())
        .with_http_mount(AppHttpMountPath::new("/second").expect("valid fixture mount"))
        .with_dependency(RuntimeAppDependency::from_app_name(first_name));

    assert!(matches!(
        collect_apps(vec![first, second]),
        Err(ServerRuntimeError::AppComposition {
            reason: RuntimeAppCompositionErrorReason::DependencyCycle,
        })
    ));
}
