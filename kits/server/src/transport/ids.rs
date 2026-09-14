// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fmt;

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

/// Identifier parsing failures for request and trace IDs.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierValueError {
    /// The value is not a valid UUID.
    #[error("identifier value is not a valid uuid")]
    InvalidUuid,
}

/// Typed request identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct RequestId(Uuid);

impl RequestId {
    /// Creates a fresh request identifier.
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an already-validated UUID as a request identifier.
    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the raw UUID value.
    pub fn into_uuid(self) -> Uuid {
        self.0
    }

    /// Parses a request identifier from canonical UUID text.
    pub fn parse_str(value: &str) -> Result<Self, IdentifierValueError> {
        let value = Uuid::parse_str(value).map_err(|_| IdentifierValueError::InvalidUuid)?;
        Ok(Self::from_uuid(value))
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Typed trace identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct TraceId(Uuid);

impl TraceId {
    /// Creates a fresh trace identifier.
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an already-validated UUID as a trace identifier.
    pub fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    /// Returns the raw UUID value.
    pub fn into_uuid(self) -> Uuid {
        self.0
    }

    /// Parses a trace identifier from canonical UUID text.
    pub fn parse_str(value: &str) -> Result<Self, IdentifierValueError> {
        let value = Uuid::parse_str(value).map_err(|_| IdentifierValueError::InvalidUuid)?;
        Ok(Self::from_uuid(value))
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
