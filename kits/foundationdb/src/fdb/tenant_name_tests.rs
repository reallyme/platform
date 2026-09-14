// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    FoundationDbTenantName, FoundationDbTenantNameError, FoundationDbTenantNameErrorReason,
    MAX_FOUNDATIONDB_TENANT_NAME_LENGTH,
};

#[test]
fn accepts_stable_operational_name() {
    let result = FoundationDbTenantName::new("search-crawl-v1");
    assert!(result.is_ok());
    let Ok(name) = result else {
        return;
    };
    assert_eq!(name.as_bytes(), b"search-crawl-v1");
    assert_eq!(name.to_string(), "search-crawl-v1");
}

#[test]
fn rejects_invalid_candidates() {
    let cases = [
        ("", FoundationDbTenantNameErrorReason::Empty),
        (
            "-tenant",
            FoundationDbTenantNameErrorReason::InvalidBoundary,
        ),
        (
            "tenant-",
            FoundationDbTenantNameErrorReason::InvalidBoundary,
        ),
        (
            "Tenant",
            FoundationDbTenantNameErrorReason::InvalidCharacter,
        ),
        (
            "tenant_name",
            FoundationDbTenantNameErrorReason::InvalidCharacter,
        ),
    ];

    for (candidate, expected_reason) in cases {
        assert_eq!(
            FoundationDbTenantName::new(candidate),
            Err(FoundationDbTenantNameError::Invalid {
                reason: expected_reason,
            })
        );
    }
}

#[test]
fn rejects_oversized_candidate() {
    let candidate = "a".repeat(MAX_FOUNDATIONDB_TENANT_NAME_LENGTH + 1);
    assert_eq!(
        FoundationDbTenantName::new(candidate.as_str()),
        Err(FoundationDbTenantNameError::Invalid {
            reason: FoundationDbTenantNameErrorReason::TooLong,
        })
    );
}
