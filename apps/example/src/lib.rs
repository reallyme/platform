// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Example app used by local and CI server-runtime verification.
//!
//! This app intentionally has no product behavior. It demonstrates the same
//! app shape future product apps should use: host-neutral use-cases, typed
//! config, app-kit descriptors, and thin host/transport adapters.

pub mod adapters;
pub mod app;
pub mod ports;
