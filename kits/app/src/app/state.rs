// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Generic host-neutral app state wrapper.
///
/// Stores validated app-owned configuration without acquiring runtime or
/// infrastructure responsibilities. Apps with additional host-neutral resources
/// can supply their own state type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardAppState<TConfig> {
    config: TConfig,
}

impl<TConfig> StandardAppState<TConfig> {
    /// Constructs app state from validated app config.
    pub const fn new(config: TConfig) -> Self {
        Self { config }
    }

    /// Returns the validated app config.
    pub const fn config(&self) -> &TConfig {
        &self.config
    }
}
