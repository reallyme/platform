// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Desired-state DNS model for Hephaestus-managed servers.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

use super::HephaestusDomainError;

const MAX_DNS_NAME_BYTES: usize = 253;
const MAX_DNS_TARGET_BYTES: usize = 45;
const MAX_DNS_RECORDS: usize = 256;

/// DNS record kind managed by Hephaestus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HephaestusDnsRecordKind {
    /// IPv4 address record.
    A,
    /// IPv6 address record.
    Aaaa,
}

/// DNS record visibility used by the fleet dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HephaestusDnsRecordVisibility {
    /// Public DNS entry, usually `service.example.com -> server public IP`.
    Public,
    /// Private DNS entry, usually `internal name -> private IP`.
    Internal,
}

/// Fully-qualified or internal DNS record name.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HephaestusDnsRecordName(String);

impl HephaestusDnsRecordName {
    /// Constructs a validated DNS record name.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim().trim_end_matches('.');
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_DNS_NAME_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if trimmed.starts_with('-')
            || trimmed.ends_with('-')
            || trimmed.starts_with('.')
            || trimmed.ends_with('.')
            || trimmed.contains("..")
            || !trimmed.bytes().all(is_dns_name_byte)
        {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_ascii_lowercase()))
    }

    /// Returns the normalized DNS name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusDnsRecordName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusDnsRecordName")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusDnsRecordName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// DNS record target address.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct HephaestusDnsRecordTarget(String);

impl HephaestusDnsRecordTarget {
    /// Constructs a validated IP target.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_DNS_TARGET_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_ip_address_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the target IP address.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Debug for HephaestusDnsRecordTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("HephaestusDnsRecordTarget")
            .field(&self.0)
            .finish()
    }
}

impl<'de> Deserialize<'de> for HephaestusDnsRecordTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// One desired DNS record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusDnsRecord {
    name: HephaestusDnsRecordName,
    kind: HephaestusDnsRecordKind,
    target: HephaestusDnsRecordTarget,
    visibility: HephaestusDnsRecordVisibility,
}

impl HephaestusDnsRecord {
    /// Constructs a desired DNS record.
    pub const fn new(
        name: HephaestusDnsRecordName,
        kind: HephaestusDnsRecordKind,
        target: HephaestusDnsRecordTarget,
        visibility: HephaestusDnsRecordVisibility,
    ) -> Self {
        Self {
            name,
            kind,
            target,
            visibility,
        }
    }

    /// Returns the DNS record name.
    pub const fn name(&self) -> &HephaestusDnsRecordName {
        &self.name
    }

    /// Returns the DNS record kind.
    pub const fn kind(&self) -> HephaestusDnsRecordKind {
        self.kind
    }

    /// Returns the DNS target address.
    pub const fn target(&self) -> &HephaestusDnsRecordTarget {
        &self.target
    }

    /// Returns whether this record is public or internal.
    pub const fn visibility(&self) -> HephaestusDnsRecordVisibility {
        self.visibility
    }
}

/// Complete desired DNS state for Hephaestus-managed names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HephaestusDnsDesiredState {
    records: Vec<HephaestusDnsRecord>,
}

impl HephaestusDnsDesiredState {
    /// Constructs a desired DNS state document.
    pub fn new(records: Vec<HephaestusDnsRecord>) -> Result<Self, HephaestusDomainError> {
        if records.len() > MAX_DNS_RECORDS {
            return Err(HephaestusDomainError::TooLong);
        }
        Ok(Self { records })
    }

    /// Returns desired DNS records.
    pub fn records(&self) -> &[HephaestusDnsRecord] {
        self.records.as_slice()
    }
}

impl<'de> Deserialize<'de> for HephaestusDnsDesiredState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            records: Vec<HephaestusDnsRecord>,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(wire.records).map_err(serde::de::Error::custom)
    }
}

fn is_dns_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')
}

fn is_ip_address_byte(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'.' | b':')
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fixed test fixtures should fail loudly if validation invariants change"
)]
mod tests {
    use super::{
        HephaestusDnsDesiredState, HephaestusDnsRecord, HephaestusDnsRecordKind,
        HephaestusDnsRecordName, HephaestusDnsRecordTarget, HephaestusDnsRecordVisibility,
    };

    #[test]
    fn dns_name_normalizes_to_lowercase_without_trailing_dot() {
        let name =
            HephaestusDnsRecordName::new("Service.Example.COM.").expect("valid dns name fixture");

        assert_eq!(name.as_str(), "service.example.com");
    }

    #[test]
    fn dns_desired_state_models_public_and_internal_records() {
        let public = HephaestusDnsRecord::new(
            HephaestusDnsRecordName::new("service.example.com").expect("valid public name"),
            HephaestusDnsRecordKind::A,
            HephaestusDnsRecordTarget::new("203.0.113.10").expect("valid public ip"),
            HephaestusDnsRecordVisibility::Public,
        );
        let internal = HephaestusDnsRecord::new(
            HephaestusDnsRecordName::new("service.internal").expect("valid internal name"),
            HephaestusDnsRecordKind::A,
            HephaestusDnsRecordTarget::new("10.1.1.10").expect("valid private ip"),
            HephaestusDnsRecordVisibility::Internal,
        );

        let state =
            HephaestusDnsDesiredState::new(vec![public, internal]).expect("valid desired state");

        assert_eq!(state.records().len(), 2);
    }
}
