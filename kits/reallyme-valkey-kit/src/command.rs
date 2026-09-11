// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generic app-owned command construction.

/// A Valkey command whose response is decoded by [`crate::ValkeyConnector`].
pub type ValkeyCommand = redis::Cmd;

/// A Valkey command pipeline whose response is decoded by
/// [`crate::ValkeyConnector`].
pub type ValkeyPipeline = redis::Pipeline;

/// Starts a command with a compile-time command name.
///
/// Restricting the name to a static string ensures external input cannot select
/// administrative commands. Applications should add untrusted values only as
/// encoded arguments and namespace every application-owned key with
/// [`crate::ValkeyConnector::namespaced_key`].
pub fn valkey_command(name: &'static str) -> ValkeyCommand {
    redis::cmd(name)
}

/// Starts a non-atomic command pipeline.
///
/// Call [`ValkeyPipeline::atomic`] when all commands must execute as one Valkey
/// transaction. Pipeline construction stays app-owned because only the app can
/// define the operation's atomicity and idempotency requirements.
pub fn valkey_pipeline() -> ValkeyPipeline {
    redis::pipe()
}

#[cfg(test)]
mod tests {
    use super::{valkey_command, valkey_pipeline};

    #[test]
    fn command_and_pipeline_factories_construct_driver_values() {
        let mut command = valkey_command("PING");
        command.arg("bounded");

        let mut pipeline = valkey_pipeline();
        pipeline.cmd("PING");
        pipeline.atomic();
    }
}
