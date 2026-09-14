// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitField};
use crate::name_validator::{NameValidation, validate_name};

const MAX_METRIC_NAME_BYTES: usize = 64;

/// Validated app metric name.
///
/// App metric names are labels under stable app-kit metric instruments rather
/// than dynamically-created metric instruments. This keeps app metrics
/// auditable and avoids accidental metric-name cardinality explosions.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppMetricName(String);

impl AppMetricName {
    /// Constructs a validated app metric name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_metric_token(value.as_str(), AppKitField::MetricName)?;

        Ok(Self(value))
    }

    /// Returns the validated metric name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppMetricName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppMetricName")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppMetricName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppMetricName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

pub(crate) fn validate_metric_token(value: &str, field: AppKitField) -> Result<(), AppKitError> {
    let rules = NameValidation::new(b"_", b"_", MAX_METRIC_NAME_BYTES, false);
    validate_name(value, field, &rules)
}

#[cfg(test)]
#[path = "metric_name_tests.rs"]
mod tests;
