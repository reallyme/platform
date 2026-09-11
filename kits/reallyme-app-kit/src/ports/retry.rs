// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Host-neutral retry posture for app downstream ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPortRetryPolicy {
    /// Do not retry the operation.
    NoRetry,
    /// Retry only when the operation is known to be idempotent.
    IdempotentOnly,
}
