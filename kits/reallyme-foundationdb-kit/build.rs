// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

fn main() {
    // FoundationDB ships a separate C client library. We add the common macOS install
    // directory explicitly so local builds do not depend on ad hoc shell environment state.
    let candidates = ["/usr/local/lib", "/opt/homebrew/lib"];

    for candidate in candidates {
        let dylib = Path::new(candidate).join("libfdb_c.dylib");
        if dylib.exists() {
            println!("cargo:rustc-link-search=native={candidate}");
            break;
        }
    }
}
