// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_foundationdb_kit::FoundationDbTenantName;

use super::{AdminToolError, Command, parse_command, parse_expected_tenant_id, parse_tenant};

#[cfg(unix)]
#[path = "fdb-tenant-admin/live_tests.rs"]
mod live;

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

#[test]
fn parses_recorded_delete_and_resume_without_changing_existing_commands() {
    let parse = |args: &[&str]| parse_command(args.iter().map(std::ffi::OsString::from));
    assert!(matches!(
        parse(&["delete", "catalog"]),
        Ok(Command::Delete(_))
    ));
    assert!(
        matches!(parse(&["delete", "catalog", "--record", "a path"]),
        Ok(Command::RecordedDelete(_, path)) if path == std::path::Path::new("a path"))
    );
    assert!(matches!(parse(&["resume-delete", "a path"]),
        Ok(Command::ResumeDelete(path)) if path == std::path::Path::new("a path")));
    assert!(matches!(parse(&["prune-delete", "a path"]),
        Ok(Command::PruneDelete(path)) if path == std::path::Path::new("a path")));
    assert!(matches!(
        parse(&["takeover-delete", "catalog"]),
        Ok(Command::TakeoverDelete(_))
    ));
    assert!(matches!(
        parse(&["delete-cutoff", "catalog"]),
        Ok(Command::DeleteCutoff(_))
    ));
    assert!(matches!(
        parse(&["advance-delete-cutoff", "catalog", "2"]),
        Ok(Command::AdvanceDeleteCutoff(_, generation)) if generation.value() == 2
    ));
    for invalid in ["0", "-1", "+1", "18446744073709551616"] {
        assert!(matches!(
            parse(&["advance-delete-cutoff", "catalog", invalid]),
            Err(AdminToolError::Reservation(
                super::ReservationErrorReason::InvalidCutoffTarget
            ))
        ));
    }
    for args in [
        &["delete", "catalog", "--record"][..],
        &["delete", "catalog", "--wrong", "path"],
        &["resume-delete"],
        &["resume-delete", "path", "unexpected"],
        &["prune-delete"],
        &["prune-delete", "path", "unexpected"],
        &["advance-delete-cutoff", "catalog"],
    ] {
        assert!(matches!(parse(args), Err(AdminToolError::Usage)));
    }
}
