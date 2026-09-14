// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Host-neutral downstream port health state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPortHealth {
    /// Downstream port is reachable and ready for app use.
    Ready,
    /// Downstream port is known unavailable.
    Unavailable,
    /// Downstream port has not been checked.
    Unknown,
}
