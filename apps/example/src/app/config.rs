// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::AppConfig;

/// Typed example app behavior config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExampleAppConfig {
    hello_enabled: bool,
}

impl ExampleAppConfig {
    /// Constructs validated example app config.
    pub const fn new(hello_enabled: bool) -> Self {
        Self { hello_enabled }
    }

    /// Returns whether the hello use-case is enabled.
    pub const fn hello_enabled(self) -> bool {
        self.hello_enabled
    }
}

impl AppConfig for ExampleAppConfig {
    type Error = std::convert::Infallible;

    fn validate(self) -> Result<Self, Self::Error> {
        // The only behavior setting is already a bool. The JSONC boundary
        // rejects any value that cannot be represented by this type.
        Ok(self)
    }
}
