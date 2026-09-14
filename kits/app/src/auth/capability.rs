// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitField};

use super::validate_symbolic_name;

const MAX_CAPABILITY_NAME_BYTES: usize = 128;

/// Validated app capability name.
///
/// Capabilities describe app-exposed features or host requirements at a coarse
/// level, for example `http`, `grpc`, or `connect_rpc`. They are not product
/// permissions and must not encode user-specific or tenant-specific values.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppCapabilityName(String);

impl AppCapabilityName {
    /// Constructs a validated app capability name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_symbolic_name(
            value.as_str(),
            AppKitField::CapabilityName,
            MAX_CAPABILITY_NAME_BYTES,
        )?;

        Ok(Self(value))
    }

    /// Returns the capability name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppCapabilityName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppCapabilityName")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppCapabilityName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppCapabilityName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Host-neutral app capability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AppCapability {
    name: AppCapabilityName,
}

impl AppCapability {
    /// Constructs a capability from a validated capability name.
    pub const fn new(name: AppCapabilityName) -> Self {
        Self { name }
    }

    /// Returns the capability name.
    pub const fn name(&self) -> &AppCapabilityName {
        &self.name
    }
}

#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
