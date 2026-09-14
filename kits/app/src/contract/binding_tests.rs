// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AppDependencyBindingKey, AppDependencyBindingMode};
use crate::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn parses_dependency_binding_keys() {
    let key = AppDependencyBindingKey::parse("api.handle").expect("valid dependency key");

    assert_eq!(key.source_app().as_str(), "api");
    assert_eq!(key.port_name().as_str(), "handle");
}

#[test]
fn rejects_malformed_dependency_binding_keys() {
    assert_eq!(
        AppDependencyBindingKey::parse("api.handle.extra"),
        Err(AppKitError::new(
            AppKitField::DependencyBinding,
            AppKitErrorReason::InvalidCharacter,
        )),
    );
}

#[test]
fn dependency_binding_modes_are_stable() {
    assert_eq!(
        AppDependencyBindingMode::InProcess,
        AppDependencyBindingMode::InProcess,
    );
}
