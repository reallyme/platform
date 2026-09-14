// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

//! Reference native-server composition for ReallyMe Platform.
//!
//! This crate demonstrates the host boundary: the binary selects its apps at
//! compile time, supplies validated process configuration, and delegates
//! listeners, operational routes, task supervision, and shutdown to
//! `reallyme-server-kit`.

mod cli;
mod composition;
mod config;
mod error;
mod registry;

pub use composition::{build_runtime, run};
pub use error::{ExampleServerError, ExampleServerErrorReason};
