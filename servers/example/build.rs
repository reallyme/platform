// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=GIT_SHA");
    println!("cargo:rerun-if-env-changed=BUILD_TIMESTAMP");
    println!("cargo:rerun-if-env-changed=RUSTC_VERSION");

    if let Ok(profile) = env::var("PROFILE")
        && profile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        // Cargo's profile is safe, low-cardinality build metadata. Git revision,
        // timestamp, and compiler version remain explicit build-system inputs
        // so local builds stay reproducible and never shell out implicitly.
        println!("cargo:rustc-env=BUILD_PROFILE={profile}");
    }
}
