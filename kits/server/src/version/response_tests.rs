// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::VersionResponse;
use crate::startup::ServerName;
use crate::version::BuildInfo;

#[test]
fn response_serialization_shape_is_stable() {
    let server_name = ServerName::new("reallyme-api").expect("valid server name");
    let build_info = BuildInfo::from_parts(
        server_name,
        "1.2.3",
        Some("abc1234"),
        None,
        Some("release"),
        None,
    );
    let response = VersionResponse::from(build_info);

    let serialized =
        serde_json::to_value(response).expect("version response should serialize to json");

    assert_eq!(
        serialized,
        json!({
            "server_name": "reallyme-api",
            "service_version": "1.2.3",
            "git_sha": "abc1234",
            "build_timestamp": null,
            "build_profile": "release",
            "rustc_version": null
        })
    );
}
