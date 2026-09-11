// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;

use super::super::{AppName, RuntimeApp, collect_apps};
use crate::runtime::{RuntimeAppCompositionErrorReason, ServerRuntimeError};

#[test]
fn runtime_app_collection_rejects_duplicate_http_mounts() {
    let first = RuntimeApp::new(
        AppName::new("api").expect("valid fixture app name"),
        Router::new(),
    );
    let second = RuntimeApp::new(
        AppName::new("test").expect("valid fixture app name"),
        Router::new(),
    );

    assert!(matches!(
        collect_apps(vec![first, second]),
        Err(ServerRuntimeError::AppComposition {
            reason: RuntimeAppCompositionErrorReason::DuplicateHttpMount,
        })
    ));
}
