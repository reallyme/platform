// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::StartupError;

/// Validated deployment/region label used in safe startup summaries.
///
/// Region is operational metadata, not a trust signal. It is deliberately
/// bounded and low-cardinality so it is safe for structured logs and future
/// metrics labels when deployments need regional observability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentRegion(String);

impl DeploymentRegion {
    /// Constructs a validated deployment region.
    pub fn new(value: impl Into<String>) -> Result<Self, StartupError> {
        let value = value.into();

        if value.is_empty() {
            return Err(StartupError::EmptyDeploymentRegion);
        }

        if value.len() > 63 {
            return Err(StartupError::DeploymentRegionTooLong);
        }

        if value.starts_with('-') || value.ends_with('-') {
            return Err(StartupError::InvalidDeploymentRegionBoundary);
        }

        if value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Ok(Self(value));
        }

        Err(StartupError::InvalidDeploymentRegion)
    }

    /// Returns the validated deployment region.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Default for DeploymentRegion {
    fn default() -> Self {
        Self("unknown".to_owned())
    }
}
