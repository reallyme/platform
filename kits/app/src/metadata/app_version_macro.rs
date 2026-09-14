// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Constructs an app version from the calling crate's Cargo metadata.
///
/// This macro intentionally resolves `CARGO_PKG_VERSION` in the caller's crate
/// rather than this crate's metadata.
#[macro_export]
macro_rules! app_version {
    () => {
        $crate::metadata::AppVersion::new(env!("CARGO_PKG_VERSION"))
    };
}
