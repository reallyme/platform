// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
    let announced = AclTagSet::parse(["prod", "vultr", "eu"]).expect("announced tags should parse");
    let required = AclTagSet::parse(["prod", "eu"]).expect("required tags should parse");

    assert!(announced.contains_required(&required));
}

#[test]
fn acl_tag_set_rejects_oversized_sets() {
    let values = [
        "tag00", "tag01", "tag02", "tag03", "tag04", "tag05", "tag06", "tag07", "tag08", "tag09",
        "tag10", "tag11", "tag12", "tag13", "tag14", "tag15", "tag16", "tag17", "tag18", "tag19",
        "tag20", "tag21", "tag22", "tag23", "tag24", "tag25", "tag26", "tag27", "tag28", "tag29",
        "tag30", "tag31", "tag32",
    ];
    let error = AclTagSet::parse(values).expect_err("oversized tag set should fail");

    assert_eq!(error.reason(), AclTagErrorReason::TooManyTags);
}
