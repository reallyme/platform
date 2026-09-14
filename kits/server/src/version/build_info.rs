// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::startup::ServerName;

const UNKNOWN_BUILD_VALUE: &str = "unknown";

/// Immutable build metadata exposed by service info endpoints.
///
/// This type is intended to remain safe for public `/version` responses and
/// other operator-facing build identity surfaces. It must never include
/// secrets, credentials, tokens, raw environment values, or other sensitive
/// runtime state.
///
/// Build metadata is sourced strictly from compile-time environment variables
/// or explicit constructor inputs. This type never reads the filesystem and
/// never shells out to Git or other local tooling at runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInfo {
    /// Validated server name.
    server_name: String,
    /// Semantic version of the current crate.
    service_version: &'static str,
    /// Source control revision identifier, if available.
    git_sha: Option<&'static str>,
    /// Build timestamp, if available.
    build_timestamp: Option<&'static str>,
    /// Cargo profile or equivalent build profile, if provided at build time.
    build_profile: Option<&'static str>,
    /// Rust compiler or toolchain version, if provided at build time.
    rustc_version: Option<&'static str>,
}

impl BuildInfo {
    /// Constructs build metadata for the current crate.
    pub fn new(server_name: ServerName) -> Self {
        Self::from_parts(
            server_name,
            env!("CARGO_PKG_VERSION"),
            option_env!("GIT_SHA"),
            option_env!("BUILD_TIMESTAMP"),
            option_env!("BUILD_PROFILE"),
            option_env!("RUSTC_VERSION"),
        )
    }

    /// Constructs build metadata from explicit compile-time-safe inputs.
    ///
    /// This constructor exists for tests and for build systems that want to
    /// pass validated build identity explicitly rather than relying on the
    /// default compile-time environment variables.
    pub fn from_parts(
        server_name: ServerName,
        service_version: &'static str,
        git_sha: Option<&'static str>,
        build_timestamp: Option<&'static str>,
        build_profile: Option<&'static str>,
        rustc_version: Option<&'static str>,
    ) -> Self {
        Self {
            server_name: server_name.as_str().to_owned(),
            service_version,
            git_sha,
            build_timestamp,
            build_profile,
            rustc_version,
        }
    }

    /// Returns the validated server name.
    pub fn server_name(&self) -> &str {
        self.server_name.as_str()
    }

    /// Returns the semantic version of the current crate.
    pub const fn service_version(&self) -> &'static str {
        self.service_version
    }

    /// Returns the source-control revision identifier, if provided.
    pub const fn git_sha(&self) -> Option<&'static str> {
        self.git_sha
    }

    /// Returns the source-control revision identifier or a safe fallback.
    pub const fn git_sha_or_unknown(&self) -> &'static str {
        match self.git_sha {
            Some(value) => value,
            None => UNKNOWN_BUILD_VALUE,
        }
    }

    /// Returns the build timestamp, if provided.
    pub const fn build_timestamp(&self) -> Option<&'static str> {
        self.build_timestamp
    }

    /// Returns the build timestamp or a safe fallback.
    pub const fn build_timestamp_or_unknown(&self) -> &'static str {
        match self.build_timestamp {
            Some(value) => value,
            None => UNKNOWN_BUILD_VALUE,
        }
    }

    /// Returns the build profile, if provided.
    pub const fn build_profile(&self) -> Option<&'static str> {
        self.build_profile
    }

    /// Returns the Rust compiler/toolchain version, if provided.
    pub const fn rustc_version(&self) -> Option<&'static str> {
        self.rustc_version
    }
}

#[cfg(test)]
#[path = "build_info_tests.rs"]
mod tests;
