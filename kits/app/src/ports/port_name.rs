// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::auth::validate_symbolic_name;
use crate::error::{AppKitError, AppKitField};

const MAX_PORT_NAME_BYTES: usize = 128;

/// Validated app port name.
///
/// Ports are host-neutral downstream boundaries such as
/// `handle_policy_client` or `verification_client`. App-kit does not define
/// the concrete port traits because those belong to the app or domain layer.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppPortName(String);

impl AppPortName {
    /// Constructs a validated app port name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_symbolic_name(value.as_str(), AppKitField::PortName, MAX_PORT_NAME_BYTES)?;

        Ok(Self(value))
    }

    /// Returns the validated port name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppPortName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("AppPortName").field(&self.0).finish()
    }
}

impl Serialize for AppPortName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppPortName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
#[path = "port_name_tests.rs"]
mod tests;
