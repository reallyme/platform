// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{future::Future, pin::Pin};

/// Host-neutral shutdown observation contract for app background work.
///
/// Native server hosts can back this with cancellation tokens. Worker hosts can
/// back it with platform request/shutdown state. Apps must not assume a concrete
/// runtime implementation.
pub trait AppTaskShutdown: Send + Sync {
    /// Returns a shutdown-aware future that resolves once cancellation is requested.
    fn shutdown_signal(&self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;

    /// Returns whether the host has requested shutdown/cancellation.
    fn is_shutdown_requested(&self) -> bool;
}
