// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

//! Reference host-neutral application with native and Worker adapters.
//!
//! The Hello use-case demonstrates typed configuration, app-kit descriptors,
//! and transport adaptation without owning process or host lifecycle.

pub mod adapters;
pub mod app;
pub mod ports;
