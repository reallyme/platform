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
fn profile_file_names_are_stable() {
    assert_eq!(AppConfigProfile::Local.jsonc_file_name(), "local.jsonc");
    assert_eq!(AppConfigProfile::Staging.jsonc_file_name(), "staging.jsonc");
    assert_eq!(AppConfigProfile::Prod.jsonc_file_name(), "prod.jsonc");
}
