// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppPortDescriptor;

/// Host-neutral downstream port descriptor.
///
/// Downstream ports are app-owned outbound boundaries: another app, a remote
/// capability, or an external provider that the app calls through a typed
/// contract. Server composition may later bind that port in-process or to a
/// remote Connect client, but the app contract should describe it as a
/// downstream port rather than as a vague "dependency".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDownstreamPortDescriptor {
    port: AppPortDescriptor,
}

impl AppDownstreamPortDescriptor {
    /// Constructs a downstream descriptor from a generic port descriptor.
    pub const fn new(port: AppPortDescriptor) -> Self {
        Self { port }
    }

    /// Returns the underlying port descriptor.
    pub const fn port(&self) -> &AppPortDescriptor {
        &self.port
    }
}
