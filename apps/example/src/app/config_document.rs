// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "config_document_tests.rs"]
mod tests;
