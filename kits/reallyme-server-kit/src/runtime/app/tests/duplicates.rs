// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;

use super::super::{AppHttpMountPath, AppName, RuntimeApp, collect_apps};
use crate::runtime::{RuntimeAppCompositionErrorReason, ServerRuntimeError};

#[test]
fn runtime_app_collection_rejects_duplicate_names() {
    let first = RuntimeApp::new(
        AppName::new("api").expect("valid fixture app name"),
        Router::new(),
    );
    let second = RuntimeApp::new(
        AppName::new("api").expect("valid fixture app name"),
        Router::new(),
    )
    .with_http_mount(AppHttpMountPath::new("/second").expect("valid fixture HTTP mount path"));

    assert!(matches!(
        collect_apps(vec![first, second]),
        Err(ServerRuntimeError::AppComposition {
            reason: RuntimeAppCompositionErrorReason::DuplicateAppName,
        })
    ));
}

#[test]
fn duplicate_dependency_fails_closed() {
    let dependency = RuntimeApp::new(
        AppName::new("dependency").expect("valid fixture app name"),
        Router::new(),
    );
    let handle = dependency.handle();
    let dependent = RuntimeApp::new(
        AppName::new("dependent").expect("valid fixture app name"),
        Router::new(),
    )
    .with_http_mount(AppHttpMountPath::new("/dependent").expect("valid fixture mount"))
    .with_dependency(super::super::RuntimeAppDependency::from_handle(&handle))
    .with_dependency(super::super::RuntimeAppDependency::from_handle(&handle));

    assert!(matches!(
        collect_apps(vec![dependency, dependent]),
        Err(ServerRuntimeError::AppComposition {
            reason: RuntimeAppCompositionErrorReason::DuplicateDependency,
        })
    ));
}
