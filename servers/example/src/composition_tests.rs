// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;

use super::build_runtime;

#[test]
fn reference_server_composition_is_valid() {
    assert!(build_runtime(config_path().as_path()).is_ok());
}

fn config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/example-server.jsonc")
}
