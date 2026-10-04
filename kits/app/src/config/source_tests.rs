// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AppConfigFormat, AppConfigProfile, AppConfigSource};
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn accepts_safe_config_source_names() {
    let source =
        AppConfigSource::new(AppConfigFormat::Jsonc, "local.jsonc").expect("valid source name");

    assert_eq!(source.name(), "local.jsonc");
}

#[test]
fn rejects_config_source_with_whitespace() {
    assert_eq!(
        AppConfigSource::new(AppConfigFormat::Jsonc, "local config.jsonc"),
        Err(AppKitError::new(
            AppKitField::ConfigSource,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}

#[test]
fn deserialization_preserves_source_name_validation() {
    for invalid in ["", "local config.jsonc"] {
        let value = serde_json::json!({"format": "jsonc", "name": invalid});
        assert!(serde_json::from_value::<AppConfigSource>(value).is_err());
    }

    let valid = serde_json::json!({"format": "jsonc", "name": "local.jsonc"});
    let source: AppConfigSource =
        serde_json::from_value(valid).expect("valid source should deserialize");
    assert_eq!(source.name(), "local.jsonc");
}

#[test]
fn profile_file_names_are_stable() {
    assert_eq!(AppConfigProfile::Local.jsonc_file_name(), "local.jsonc");
    assert_eq!(AppConfigProfile::Staging.jsonc_file_name(), "staging.jsonc");
    assert_eq!(AppConfigProfile::Prod.jsonc_file_name(), "prod.jsonc");
}
