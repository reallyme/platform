// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::{
    AppErrorCategory, AppErrorCode, AppErrorContract, AppErrorRetryDisposition,
};
use thiserror::Error;

/// Low-cardinality example app error kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExampleAppErrorKind {
    /// The hello use-case is disabled by app config.
    HelloDisabled,
    /// Static app metric descriptors are invalid.
    MetricConfigurationInvalid,
    /// The caller's deadline elapsed before the use-case could run.
    DeadlineExceeded,
}

/// Host-neutral example app error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExampleAppError {
    /// The hello use-case is disabled by app config.
    #[error("example app hello is disabled")]
    HelloDisabled,
    /// Static app metric descriptors are invalid.
    #[error("example app metric configuration is invalid")]
    MetricConfigurationInvalid,
    /// The caller's deadline elapsed before the use-case could run.
    #[error("example app deadline exceeded")]
    DeadlineExceeded,
}

impl ExampleAppError {
    /// Returns the low-cardinality error kind.
    pub const fn kind(self) -> ExampleAppErrorKind {
        match self {
            Self::HelloDisabled => ExampleAppErrorKind::HelloDisabled,
            Self::MetricConfigurationInvalid => ExampleAppErrorKind::MetricConfigurationInvalid,
            Self::DeadlineExceeded => ExampleAppErrorKind::DeadlineExceeded,
        }
    }

    /// Returns the host-neutral public app error contract.
    ///
    /// The hardcoded error code below is static and does not fail at runtime in this
    /// application today, but callers retain compatibility with the fallible
    /// constructor until const-validated kit constructors are introduced.
    pub fn contract(self) -> Result<AppErrorContract, reallyme_app_kit::AppKitError> {
        let code = match self {
            Self::HelloDisabled => AppErrorCode::new("hello_disabled")?,
            Self::MetricConfigurationInvalid => AppErrorCode::new("metric_configuration_invalid")?,
            Self::DeadlineExceeded => AppErrorCode::new("deadline_exceeded")?,
        };

        let category = match self {
            Self::HelloDisabled => AppErrorCategory::PermissionDenied,
            Self::MetricConfigurationInvalid => AppErrorCategory::Internal,
            Self::DeadlineExceeded => AppErrorCategory::DeadlineExceeded,
        };

        Ok(AppErrorContract::new(
            code,
            category,
            AppErrorRetryDisposition::NotRetryable,
        ))
    }
}
