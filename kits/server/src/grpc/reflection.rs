// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::ServiceEnvironment;

use super::error::GrpcReflectionError;

/// Re-exported tonic reflection builder used to register descriptor sets.
///
/// The builder is exposed with a `'static` descriptor lifetime because shared
/// service descriptors are expected to come from embedded generated
/// descriptor-set bytes rather than ephemeral request data.
pub type ReflectionServiceBuilder = tonic_reflection::server::Builder<'static>;

/// gRPC reflection exposure mode.
///
/// Reflection is useful during local development, but it exposes service and
/// message metadata that can materially help an attacker enumerate an API
/// surface. Server-kit therefore rejects reflection in staging and production
/// even if a caller asks for it explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GrpcReflectionMode {
    /// Reflection is disabled.
    #[default]
    Disabled,
    /// Reflection is only allowed in local/development environments.
    DevelopmentOnly,
}

impl GrpcReflectionMode {
    /// Returns whether reflection should be enabled for the provided
    /// environment.
    pub fn enabled_for(
        self,
        service_environment: ServiceEnvironment,
    ) -> Result<bool, GrpcReflectionError> {
        match self {
            Self::Disabled => Ok(false),
            Self::DevelopmentOnly => match service_environment {
                ServiceEnvironment::Local | ServiceEnvironment::Dev => Ok(true),
                ServiceEnvironment::Staging | ServiceEnvironment::Prod => {
                    Err(GrpcReflectionError::ReflectionNotAllowedInEnvironment {
                        service_environment,
                    })
                }
            },
        }
    }
}

/// Returns a reflection builder when reflection is enabled for the current
/// environment.
pub fn reflection_builder(
    mode: GrpcReflectionMode,
    service_environment: ServiceEnvironment,
) -> Result<Option<ReflectionServiceBuilder>, GrpcReflectionError> {
    if mode.enabled_for(service_environment)? {
        Ok(Some(tonic_reflection::server::Builder::configure()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
#[path = "reflection_tests.rs"]
mod tests;
