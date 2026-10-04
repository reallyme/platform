// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_foundationdb_kit::FoundationDbTenantName;

use super::{AdminToolError, parse_expected_tenant_id, parse_tenant};

#[test]
fn parses_valid_tenant() {
    let expected = FoundationDbTenantName::new("catalog").expect("valid tenant fixture");
    assert_eq!(parse_tenant(String::from("catalog")), Ok(expected));
}

#[test]
fn accepts_deployment_owned_tenant() {
    assert!(parse_tenant(String::from("production-search")).is_ok());
}

#[test]
fn rejects_invalid_dynamic_tenant() {
    assert_eq!(
        parse_tenant(String::from("Customer_Supplied")),
        Err(AdminToolError::InvalidTenant)
    );
}

#[test]
fn recovery_requires_a_valid_recorded_tenant_id() {
    assert_eq!(parse_expected_tenant_id(String::from("42")), Ok(42));
    for value in ["-1", "foreign", "18446744073709551616"] {
        assert_eq!(
            parse_expected_tenant_id(String::from(value)),
            Err(AdminToolError::InvalidTenantId)
        );
    }
}
