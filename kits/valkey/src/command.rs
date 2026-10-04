// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generic app-owned command construction.

use redis::{Cmd, Pipeline, ToRedisArgs};

/// A command constructed with an app-owned, compile-time command name.
pub struct ValkeyCommand {
    inner: Cmd,
}

impl ValkeyCommand {
    /// Appends an encoded argument without changing the selected command.
    pub fn arg<T: ToRedisArgs>(&mut self, value: T) -> &mut Self {
        self.inner.arg(value);
        self
    }

    pub(crate) const fn as_driver_command(&self) -> &Cmd {
        &self.inner
    }
}

/// A pipeline containing only commands constructed through [`valkey_command`].
pub struct ValkeyPipeline {
    inner: Pipeline,
}

impl ValkeyPipeline {
    /// Appends a fully constructed command.
    pub fn add_command(&mut self, command: ValkeyCommand) -> &mut Self {
        self.inner.add_command(command.inner);
        self
    }

    /// Runs the pipeline atomically as one transaction.
    pub fn atomic(&mut self) -> &mut Self {
        self.inner.atomic();
        self
    }

    pub(crate) const fn as_driver_pipeline(&self) -> &Pipeline {
        &self.inner
    }
}

/// Starts a command with a compile-time command name.
///
/// Restricting the name to a static string ensures external input cannot select
/// administrative commands. Applications should add untrusted values only as
/// encoded arguments and namespace every application-owned key with
/// [`crate::ValkeyConnector::namespaced_key`].
pub fn valkey_command(name: &'static str) -> ValkeyCommand {
    ValkeyCommand {
        inner: redis::cmd(name),
    }
}

/// Starts a non-atomic command pipeline.
///
/// Call [`ValkeyPipeline::atomic`] when all commands must execute as one Valkey
/// transaction. Pipeline construction stays app-owned because only the app can
/// define the operation's atomicity and idempotency requirements.
pub fn valkey_pipeline() -> ValkeyPipeline {
    ValkeyPipeline {
        inner: redis::pipe(),
    }
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
