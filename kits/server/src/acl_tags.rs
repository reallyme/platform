// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tailscale ACL tag validation and matching helpers.
//!
//! Service discovery uses tags as policy selectors, not descriptive metadata.
//! Keeping validation and subset matching in one module prevents each resolver
//! adapter from accidentally inventing slightly different security semantics.

use std::collections::BTreeSet;

use thiserror::Error;

const MAX_ACL_TAG_BYTES: usize = 64;
const MAX_ACL_TAGS: usize = 32;
const TAILSCALE_TAG_PREFIX: &str = "tag:";

/// Validated, normalized Tailscale ACL tag.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AclTag(String);

impl AclTag {
    /// Parses a tag token and normalizes the optional `tag:` prefix away.
    pub fn parse(value: &str) -> Result<Self, AclTagError> {
        let value = value.strip_prefix(TAILSCALE_TAG_PREFIX).unwrap_or(value);
        if value.is_empty() || value.len() > MAX_ACL_TAG_BYTES {
            return Err(AclTagError::new(AclTagErrorReason::InvalidTag));
        }
        if value.bytes().any(|byte| {
            !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-' || byte == b'_')
        }) {
            return Err(AclTagError::new(AclTagErrorReason::InvalidTag));
        }

        Ok(Self(value.to_owned()))
    }

    /// Returns the normalized tag token without the `tag:` prefix.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Validated ACL tag set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AclTagSet {
    tags: BTreeSet<AclTag>,
}

impl AclTagSet {
    /// Builds a tag set. Empty sets are allowed for announced services, but
    /// callers should reject empty required-selector sets at config boundaries.
    pub fn parse<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<Self, AclTagError> {
        let mut tags = BTreeSet::new();
        for value in values {
            if tags.len() >= MAX_ACL_TAGS {
                return Err(AclTagError::new(AclTagErrorReason::TooManyTags));
            }
            tags.insert(AclTag::parse(value)?);
        }

        Ok(Self { tags })
    }

    /// Returns true when every required tag is present in this announced set.
    pub fn contains_required(&self, required: &AclTagSet) -> bool {
        required.tags.iter().all(|tag| self.tags.contains(tag))
    }

    /// Returns the normalized tags in deterministic order.
    pub fn tags(&self) -> &BTreeSet<AclTag> {
        &self.tags
    }
}

/// Typed ACL tag validation error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("ACL tag validation failed")]
pub struct AclTagError {
    reason: AclTagErrorReason,
}

impl AclTagError {
    const fn new(reason: AclTagErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the low-cardinality validation reason.
    pub const fn reason(self) -> AclTagErrorReason {
        self.reason
    }
}

/// Low-cardinality ACL tag validation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclTagErrorReason {
    /// A tag token was empty, too long, or contained unsupported characters.
    InvalidTag,
    /// The tag set exceeded the configured bound.
    TooManyTags,
}

#[cfg(test)]
#[path = "acl_tags_tests.rs"]
mod tests;
