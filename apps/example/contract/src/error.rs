// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Low-cardinality example contract error kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleContractErrorKind {
    /// The host-neutral port has not been configured.
    Unconfigured,
    /// The request is forbidden for policy or configuration reasons.
    PermissionDenied,
    /// The example app is unavailable.
    Unavailable,
}

/// Typed example contract error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExampleContractError {
    /// The host-neutral port has not been configured.
    #[error("example contract port is unconfigured")]
    Unconfigured,
    /// The request is forbidden for policy or configuration reasons.
    #[error("example contract request forbidden")]
    PermissionDenied,
    /// The example app is unavailable.
    #[error("example contract target unavailable")]
    Unavailable,
}

impl ExampleContractError {
    /// Returns the low-cardinality error kind.
    pub const fn kind(self) -> ExampleContractErrorKind {
        match self {
            Self::Unconfigured => ExampleContractErrorKind::Unconfigured,
            Self::PermissionDenied => ExampleContractErrorKind::PermissionDenied,
            Self::Unavailable => ExampleContractErrorKind::Unavailable,
        }
    }
}
