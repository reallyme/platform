// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;

use super::ShutdownReason;

/// Shutdown mode selected after a termination signal is observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownMode {
    /// Graceful shutdown drains listeners, tasks, and cleanup hooks with the
    /// normal configured budgets.
    Graceful,
    /// Fast shutdown still remains bounded and cancellation-safe, but uses the
    /// runtime's fast shutdown budget so operators can explicitly request a
    /// shorter local/development interruption path.
    Fast,
}

impl ShutdownMode {
    /// Returns the stable structured-log/config value for this mode.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Graceful => "graceful",
            Self::Fast => "fast",
        }
    }
}

impl FromStr for ShutdownMode {
    type Err = ShutdownModeParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "graceful" => Ok(Self::Graceful),
            "fast" => Ok(Self::Fast),
            _ => Err(ShutdownModeParseError),
        }
    }
}

/// Typed parse error for shutdown mode config values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownModeParseError;

/// Policy mapping operating-system shutdown reasons to runtime shutdown modes.
///
/// The default policy preserves ReallyMe's current behavior: `SIGTERM`,
/// `CTRL+C`, and unknown shutdown reasons all use graceful shutdown. Server
/// config may explicitly choose fast behavior for local/development Ctrl-C
/// flows without changing the platform default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownPolicy {
    sigterm: ShutdownMode,
    ctrl_c: ShutdownMode,
    unknown: ShutdownMode,
}

impl Default for ShutdownPolicy {
    fn default() -> Self {
        Self::graceful()
    }
}

impl ShutdownPolicy {
    /// Returns the default all-graceful shutdown policy.
    pub const fn graceful() -> Self {
        Self {
            sigterm: ShutdownMode::Graceful,
            ctrl_c: ShutdownMode::Graceful,
            unknown: ShutdownMode::Graceful,
        }
    }

    /// Returns a policy with an explicit Ctrl-C mode override.
    pub const fn with_ctrl_c_mode(mut self, mode: ShutdownMode) -> Self {
        self.ctrl_c = mode;
        self
    }

    /// Returns a policy with an explicit SIGTERM mode override.
    pub const fn with_sigterm_mode(mut self, mode: ShutdownMode) -> Self {
        self.sigterm = mode;
        self
    }

    /// Returns the shutdown mode for a typed shutdown reason.
    pub const fn mode_for(self, reason: ShutdownReason) -> ShutdownMode {
        match reason {
            ShutdownReason::CtrlC => self.ctrl_c,
            ShutdownReason::Sigterm => self.sigterm,
            ShutdownReason::Drop => self.unknown,
            ShutdownReason::Unknown => self.unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{ShutdownMode, ShutdownPolicy};
    use crate::shutdown::ShutdownReason;

    #[test]
    fn sigterm_maps_to_graceful_by_default() {
        assert_eq!(
            ShutdownPolicy::default().mode_for(ShutdownReason::Sigterm),
            ShutdownMode::Graceful
        );
    }

    #[test]
    fn ctrl_c_maps_to_graceful_by_default() {
        assert_eq!(
            ShutdownPolicy::default().mode_for(ShutdownReason::CtrlC),
            ShutdownMode::Graceful
        );
    }

    #[test]
    fn ctrl_c_can_be_configured_as_fast() {
        let policy = ShutdownPolicy::default().with_ctrl_c_mode(ShutdownMode::Fast);

        assert_eq!(policy.mode_for(ShutdownReason::CtrlC), ShutdownMode::Fast);
        assert_eq!(
            policy.mode_for(ShutdownReason::Sigterm),
            ShutdownMode::Graceful
        );
    }

    #[test]
    fn shutdown_mode_parse_is_typed() {
        assert_eq!(
            ShutdownMode::from_str("graceful"),
            Ok(ShutdownMode::Graceful)
        );
        assert_eq!(ShutdownMode::from_str("fast"), Ok(ShutdownMode::Fast));
        assert!(ShutdownMode::from_str("immediate").is_err());
    }
}
