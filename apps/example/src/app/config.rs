// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::AppConfig;
use thiserror::Error;

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
    type Error = ExampleConfigError;

    fn validate(self) -> Result<Self, Self::Error> {
        Ok(self)
    }
}

/// Example app config validation error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExampleConfigError {
    /// Reserved for future app-owned config validation.
    #[error("example app config failed validation")]
    Invalid,
}
