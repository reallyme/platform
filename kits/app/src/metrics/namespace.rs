// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitField};

use super::metric_name::validate_metric_token;

/// Validated app metric namespace.
///
/// The namespace should normally match a bounded app name transformed into
/// metric-token form, for example `reallyme_api`. It must never contain tenant,
/// user, request, or product object identifiers.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppMetricNamespace(String);

impl AppMetricNamespace {
    /// Constructs a validated app metric namespace.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_metric_token(value.as_str(), AppKitField::MetricNamespace)?;

        Ok(Self(value))
    }

    /// Returns the validated metric namespace.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppMetricNamespace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppMetricNamespace")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppMetricNamespace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppMetricNamespace {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}
