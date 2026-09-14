// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::AppConfigProfile;

use super::{example_app_config_document, parse_example_app_config_document};

#[test]
fn checked_in_example_app_jsonc_configs_parse_and_validate() {
    for value in [
        include_str!("../../config/local.jsonc"),
        include_str!("../../config/staging.jsonc"),
        include_str!("../../config/prod.jsonc"),
    ] {
        let document =
            parse_example_app_config_document(value).expect("checked-in config is valid");
        assert!(!document.reflection_enabled());
        assert!(!document.custom().example_custom_property().is_empty());
    }
}

#[test]
fn app_config_profile_selects_checked_in_jsonc_source() {
    let document = example_app_config_document(AppConfigProfile::Local)
        .expect("checked-in local example config should parse");

    assert_eq!(document.custom().example_custom_property(), "local");
}
