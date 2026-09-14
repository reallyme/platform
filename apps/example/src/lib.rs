// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

//! Example app used by local and CI server-runtime verification.
//!
//! This app intentionally has no product behavior. It demonstrates the same
//! app shape future product apps should use: host-neutral use-cases, typed
//! config, app-kit descriptors, and thin host/transport adapters.

pub mod adapters;
pub mod app;
pub mod ports;
