// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{valkey_command, valkey_pipeline};

#[test]
fn command_and_pipeline_factories_construct_driver_values() {
    let mut command = valkey_command("PING");
    command.arg("bounded");

    let mut pipeline = valkey_pipeline();
    pipeline.cmd("PING");
    pipeline.atomic();
}
