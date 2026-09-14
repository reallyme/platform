// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::BuildInfo;
use crate::startup::ServerName;

#[test]
fn build_info_captures_server_name() {
    let server_name = ServerName::new("reallyme-api").expect("valid server name");

    let info = BuildInfo::new(server_name);

    assert_eq!(info.server_name(), "reallyme-api");
    assert_eq!(info.service_version(), env!("CARGO_PKG_VERSION"));
}

#[test]
fn missing_optional_fields_can_be_represented_safely() {
    let server_name = ServerName::new("reallyme-api").expect("valid server name");
    let info = BuildInfo::from_parts(server_name, "0.1.0", None, None, None, None);

    assert_eq!(info.git_sha(), None);
    assert_eq!(info.build_timestamp(), None);
    assert_eq!(info.build_profile(), None);
    assert_eq!(info.rustc_version(), None);
    assert_eq!(info.git_sha_or_unknown(), "unknown");
    assert_eq!(info.build_timestamp_or_unknown(), "unknown");
}
