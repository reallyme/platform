// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::Router;
use serde_json::json;

use super::super::{VERSION_PATH, version_route};
use crate::http::TestServer;
use crate::startup::ServerName;
use crate::version::BuildInfo;

#[tokio::test]
async fn version_route_returns_safe_build_info() {
    let server_name = ServerName::new("reallyme-api").expect("valid server name");
    let build_info = BuildInfo::new(server_name);
    let app = Router::new().route(VERSION_PATH, version_route(build_info.clone()));
    let server = TestServer::new(app);

    let response = server.get(VERSION_PATH).await;
    response.assert_status_ok();
    response.assert_json(&json!({
        "server_name": build_info.server_name(),
        "service_version": build_info.service_version(),
        "git_sha": build_info.git_sha(),
        "build_timestamp": build_info.build_timestamp(),
        "build_profile": build_info.build_profile(),
        "rustc_version": build_info.rustc_version()
    }));
}
