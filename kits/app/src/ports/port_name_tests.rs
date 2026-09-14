// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppPortName;
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn accepts_symbolic_port_names() {
    let name = AppPortName::new("handle_policy_client").expect("valid port name fixture");

    assert_eq!(name.as_str(), "handle_policy_client");
}

#[test]
fn rejects_port_names_with_uppercase_characters() {
    assert_eq!(
        AppPortName::new("HandlePolicyClient"),
        Err(AppKitError::new(
            AppKitField::PortName,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}
