// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Identifies which operating system signal initiated termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    /// Shutdown was initiated by `CTRL+C`.
    CtrlC,
    /// Shutdown was initiated by `SIGTERM`.
    Sigterm,
    /// Shutdown was initiated because a background task owner was dropped.
    Drop,
    /// Shutdown was initiated, but the specific reason could not be determined.
    Unknown,
}
