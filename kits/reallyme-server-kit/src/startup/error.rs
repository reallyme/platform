// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Startup-time validation failures for server-process and task identity.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum StartupError {
    /// The server name is missing.
    #[error("server name cannot be empty")]
    EmptyServerName,
    /// The server name exceeds the maximum supported length.
    #[error("server name must be 63 bytes or fewer")]
    ServerNameTooLong,
    /// The server name begins or ends with `-`.
    #[error("server name cannot begin or end with a hyphen")]
    InvalidServerNameBoundary,
    /// The server name contains characters outside the allowed subset.
    #[error("server name must contain lowercase ascii letters, digits, or hyphens")]
    InvalidServerName,
    /// The deployment region is missing.
    #[error("deployment region cannot be empty")]
    EmptyDeploymentRegion,
    /// The deployment region exceeds the maximum supported length.
    #[error("deployment region must be 63 bytes or fewer")]
    DeploymentRegionTooLong,
    /// The deployment region begins or ends with `-`.
    #[error("deployment region cannot begin or end with a hyphen")]
    InvalidDeploymentRegionBoundary,
    /// The deployment region contains characters outside the allowed subset.
    #[error("deployment region must contain lowercase ascii letters, digits, or hyphens")]
    InvalidDeploymentRegion,
    /// The runtime app name is missing.
    #[error("app name cannot be empty")]
    EmptyAppName,
    /// The runtime app name exceeds the maximum supported length.
    #[error("app name must be 63 bytes or fewer")]
    AppNameTooLong,
    /// The runtime app name begins or ends with `-`.
    #[error("app name cannot begin or end with a hyphen")]
    InvalidAppNameBoundary,
    /// The runtime app name contains characters outside the allowed subset.
    #[error("app name must contain lowercase ascii letters, digits, or hyphens")]
    InvalidAppName,
    /// The app HTTP mount path is missing.
    #[error("app HTTP mount path cannot be empty")]
    EmptyAppHttpMountPath,
    /// The app HTTP mount path exceeds the maximum supported length.
    #[error("app HTTP mount path must be 128 bytes or fewer")]
    AppHttpMountPathTooLong,
    /// The app HTTP mount path has invalid syntax.
    #[error("app HTTP mount path must be an absolute static path")]
    InvalidAppHttpMountPath,
    /// The background task name is missing.
    #[error("task name cannot be empty")]
    EmptyTaskName,
    /// The background task name exceeds the maximum supported length.
    #[error("task name must be 63 bytes or fewer")]
    TaskNameTooLong,
    /// The background task name begins or ends with `-`.
    #[error("task name cannot begin or end with a hyphen")]
    InvalidTaskNameBoundary,
    /// The background task name contains characters outside the allowed subset.
    #[error("task name must contain lowercase ascii letters, digits, or hyphens")]
    InvalidTaskName,
}
