// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
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
mod tests {
    use super::{AclTag, AclTagErrorReason, AclTagSet};

    #[test]
    fn acl_tag_normalizes_tailscale_prefix() {
        let tag = AclTag::parse("tag:prod").expect("tag prefix should parse");

        assert_eq!(tag.as_str(), "prod");
    }

    #[test]
    fn acl_tag_rejects_uppercase_tokens() {
        let error = AclTag::parse("Prod").expect_err("uppercase tags should fail closed");

        assert_eq!(error.reason(), AclTagErrorReason::InvalidTag);
    }

    #[test]
    fn acl_tag_set_matches_required_subset() {
        let announced =
            AclTagSet::parse(["prod", "vultr", "eu"]).expect("announced tags should parse");
        let required = AclTagSet::parse(["prod", "eu"]).expect("required tags should parse");

        assert!(announced.contains_required(&required));
    }

    #[test]
    fn acl_tag_set_rejects_oversized_sets() {
        let values = [
            "tag00", "tag01", "tag02", "tag03", "tag04", "tag05", "tag06", "tag07", "tag08",
            "tag09", "tag10", "tag11", "tag12", "tag13", "tag14", "tag15", "tag16", "tag17",
            "tag18", "tag19", "tag20", "tag21", "tag22", "tag23", "tag24", "tag25", "tag26",
            "tag27", "tag28", "tag29", "tag30", "tag31", "tag32",
        ];
        let error = AclTagSet::parse(values).expect_err("oversized tag set should fail");

        assert_eq!(error.reason(), AclTagErrorReason::TooManyTags);
    }
}
