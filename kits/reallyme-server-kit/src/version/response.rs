// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

use super::BuildInfo;

/// Stable public `/version` response model.
///
/// This type exists so HTTP transport helpers can expose a deliberate response
/// contract without relying on direct serialization of internal structs by
/// convention alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionResponse {
    server_name: String,
    service_version: &'static str,
    git_sha: Option<&'static str>,
    build_timestamp: Option<&'static str>,
    build_profile: Option<&'static str>,
    rustc_version: Option<&'static str>,
}

impl VersionResponse {
    /// Constructs a version response from safe build metadata.
    pub fn from_build_info(build_info: &BuildInfo) -> Self {
        Self {
            server_name: build_info.server_name().to_owned(),
            service_version: build_info.service_version(),
            git_sha: build_info.git_sha(),
            build_timestamp: build_info.build_timestamp(),
            build_profile: build_info.build_profile(),
            rustc_version: build_info.rustc_version(),
        }
    }

    /// Returns the safe build metadata server name.
    pub fn server_name(&self) -> &str {
        self.server_name.as_str()
    }

    /// Returns the safe build metadata service version.
    pub const fn service_version(&self) -> &'static str {
        self.service_version
    }

    /// Returns the safe build metadata git SHA, if available.
    pub const fn git_sha(&self) -> Option<&'static str> {
        self.git_sha
    }

    /// Returns the safe build metadata build timestamp, if available.
    pub const fn build_timestamp(&self) -> Option<&'static str> {
        self.build_timestamp
    }

    /// Returns the safe build metadata build profile, if available.
    pub const fn build_profile(&self) -> Option<&'static str> {
        self.build_profile
    }

    /// Returns the safe build metadata Rust compiler/toolchain version, if available.
    pub const fn rustc_version(&self) -> Option<&'static str> {
        self.rustc_version
    }
}

impl From<BuildInfo> for VersionResponse {
    fn from(build_info: BuildInfo) -> Self {
        Self::from_build_info(&build_info)
    }
}

#[cfg(test)]
mod tests {
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
}
