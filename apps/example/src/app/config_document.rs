// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::{
    AppConfigDocumentError, AppConfigProfile, AppJsoncConfigDocument,
    parse_app_jsonc_config_document,
};
use serde::Deserialize;

/// Example app-specific custom JSONC config.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ExampleCustomConfig {
    hello_enabled: bool,
    example_custom_property: String,
}

impl ExampleCustomConfig {
    /// Returns whether the hello use-case is enabled.
    pub const fn hello_enabled(&self) -> bool {
        self.hello_enabled
    }

    /// Returns the example-only custom property.
    pub fn example_custom_property(&self) -> &str {
        self.example_custom_property.as_str()
    }
}

/// Standard example app JSONC config document.
pub type ExampleAppConfigDocument = AppJsoncConfigDocument<ExampleCustomConfig>;

/// Parses and validates an example app JSONC config document.
pub fn parse_example_app_config_document(
    value: &str,
) -> Result<ExampleAppConfigDocument, AppConfigDocumentError> {
    parse_app_jsonc_config_document(value)
}

/// Loads the checked-in example app config document for a host-selected profile.
pub fn example_app_config_document(
    profile: AppConfigProfile,
) -> Result<ExampleAppConfigDocument, AppConfigDocumentError> {
    parse_example_app_config_document(match profile {
        AppConfigProfile::Local => include_str!("../../config/local.jsonc"),
        AppConfigProfile::Staging => include_str!("../../config/staging.jsonc"),
        AppConfigProfile::Prod => include_str!("../../config/prod.jsonc"),
    })
}

#[cfg(test)]
mod tests {
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
}
