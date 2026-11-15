// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::http_contract as wire;
use serde::{Serialize, Serializer};

/// Liveness state reported by the process.
///
/// The foundational server kit keeps liveness intentionally simple: if the
/// process is still able to respond, it is live. Liveness must not depend on
/// downstream services or external systems because it answers whether the
/// process itself should be restarted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivenessState {
    /// The process is live.
    Live,
}

impl LivenessState {
    pub(super) const fn wire_state(self) -> wire::LivenessState {
        match self {
            Self::Live => wire::LivenessState::Live,
        }
    }
}

impl Serialize for LivenessState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.wire_state().serialize(serializer)
    }
}
