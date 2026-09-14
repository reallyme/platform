// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_foundationdb_kit::FoundationDbTenantName;

use super::{AdminToolError, parse_tenant};

#[test]
fn parses_valid_tenant() {
    let expected = FoundationDbTenantName::new("catalog");
    assert!(expected.is_ok());
    let Ok(expected) = expected else {
        return;
    };
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
