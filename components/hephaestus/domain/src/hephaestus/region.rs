// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! First-class operational Region definitions for Hephaestus.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::HephaestusDomainError;

const MAX_REGION_ID_BYTES: usize = 64;
const MAX_SITE_ID_BYTES: usize = 64;
const MAX_REGION_SITES: usize = 64;

/// Stable ReallyMe operational Region identifier.
///
/// This is intentionally distinct from any infrastructure-provider vocabulary.
/// In Hephaestus, a Region is an operator-defined grouping of one or more
/// Sites that share placement, networking, and service requirements.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusRegionId(String);

impl HephaestusRegionId {
    /// Constructs a validated operational Region identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_REGION_ID_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_region_id_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated Region identifier text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Stable ReallyMe site or datacenter identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HephaestusSiteId(String);

impl HephaestusSiteId {
    /// Constructs a validated site identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, HephaestusDomainError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if trimmed.len() > MAX_SITE_ID_BYTES {
            return Err(HephaestusDomainError::TooLong);
        }
        if !trimmed.bytes().all(is_region_id_byte) {
            return Err(HephaestusDomainError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the validated site identifier text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Durable mapping from one ReallyMe operational Region to the Sites that
/// belong to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HephaestusRegionDefinition {
    region_id: HephaestusRegionId,
    site_ids: Vec<HephaestusSiteId>,
}

impl HephaestusRegionDefinition {
    /// Constructs a validated Region definition.
    pub fn new(
        region_id: HephaestusRegionId,
        site_ids: Vec<HephaestusSiteId>,
    ) -> Result<Self, HephaestusDomainError> {
        if site_ids.is_empty() {
            return Err(HephaestusDomainError::Empty);
        }
        if site_ids.len() > MAX_REGION_SITES {
            return Err(HephaestusDomainError::TooLong);
        }

        let mut seen = BTreeSet::new();
        for site_id in &site_ids {
            if !seen.insert(site_id.as_str().to_owned()) {
                return Err(HephaestusDomainError::InvalidCharacter);
            }
        }

        Ok(Self {
            region_id,
            site_ids,
        })
    }

    /// Returns the stable Region identifier.
    pub const fn region_id(&self) -> &HephaestusRegionId {
        &self.region_id
    }

    /// Returns the Sites assigned to this Region.
    pub fn site_ids(&self) -> &[HephaestusSiteId] {
        self.site_ids.as_slice()
    }

    /// Returns whether the Region contains the supplied Site.
    pub fn contains_site_id(&self, site_id: &HephaestusSiteId) -> bool {
        self.site_ids.iter().any(|value| value == site_id)
    }
}

fn is_region_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "fixed test fixtures should fail loudly if validation invariants change"
)]
mod tests {
    use super::{HephaestusRegionDefinition, HephaestusRegionId, HephaestusSiteId};

    #[test]
    fn region_definition_rejects_duplicate_sites() {
        let result = HephaestusRegionDefinition::new(
            HephaestusRegionId::new("EU").expect("valid region"),
            vec![
                HephaestusSiteId::new("ams").expect("valid site"),
                HephaestusSiteId::new("ams").expect("valid site"),
            ],
        );

        assert!(result.is_err());
    }

    #[test]
    fn region_definition_tracks_site_membership() {
        let region = HephaestusRegionDefinition::new(
            HephaestusRegionId::new("EU").expect("valid region"),
            vec![
                HephaestusSiteId::new("ams").expect("valid site"),
                HephaestusSiteId::new("cdg").expect("valid site"),
            ],
        )
        .expect("valid region definition");

        assert!(region.contains_site_id(&HephaestusSiteId::new("ams").expect("valid site")));
        assert!(!region.contains_site_id(&HephaestusSiteId::new("ewr").expect("valid site")));
    }
}
