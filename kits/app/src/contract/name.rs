// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{AppKitError, AppKitErrorReason, AppKitField};
use crate::name_validator::{NameValidation, validate_name};

const MAX_CONTRACT_NAME_BYTES: usize = 128;

/// Validated app contract name.
///
/// Contract names identify capability-owned DTO/RPC contracts such as
/// `reallyme-api-contract` or `reallyme-handle-contract`. They are deployment
/// stable, low-cardinality names and must never encode tenant, user, request,
/// handle, or environment-specific values.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AppContractName(String);

impl AppContractName {
    /// Constructs a validated app contract name.
    pub fn new(value: impl Into<String>) -> Result<Self, AppKitError> {
        let value = value.into();
        validate_contract_name(value.as_str())?;

        Ok(Self(value))
    }

    /// Returns the validated contract name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for AppContractName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("AppContractName")
            .field(&self.0)
            .finish()
    }
}

impl Serialize for AppContractName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AppContractName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

fn validate_contract_name(value: &str) -> Result<(), AppKitError> {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) {
        return Err(AppKitError::new(
            AppKitField::ContractName,
            AppKitErrorReason::InvalidCharacter,
        ));
    }

    let rules = NameValidation::new(b"-", b"-", MAX_CONTRACT_NAME_BYTES, false);
    validate_name(value, AppKitField::ContractName, &rules)
}

#[cfg(test)]
#[path = "name_tests.rs"]
mod tests;
