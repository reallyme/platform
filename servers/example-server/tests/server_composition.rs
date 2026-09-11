// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;

#[test]
fn published_example_configuration_builds_the_complete_server() {
    let config_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../configs/example-server.jsonc");

    assert!(example_server::build_runtime(config_path.as_path()).is_ok());
}
