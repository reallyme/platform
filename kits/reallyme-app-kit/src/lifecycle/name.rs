// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitField};
use crate::metadata::validate_dns_label;

/// Validated name for an app lifecycle item.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppLifecycleName(String);

impl AppLifecycleName {
    /// Constructs a validated lifecycle item name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_dns_label(value.as_str(), AppKitField::LifecycleName)?;

        Ok(Self(value))
    }

    /// Returns the lifecycle item name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppLifecycleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppLifecycleName")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppLifecycleName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppLifecycleName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}
