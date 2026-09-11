// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Serialize;

/// Liveness state reported by the process.
///
/// The foundational server kit keeps liveness intentionally simple: if the
/// process is still able to respond, it is live. Liveness must not depend on
/// downstream services or external systems because it answers whether the
/// process itself should be restarted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LivenessState {
    /// The process is live.
    Live,
}
