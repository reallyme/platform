// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Cloudflare Workers host for `example-app`.
//!
//! This crate is intentionally a host adapter, not app behavior. It owns the
//! `workers-rs` entrypoint and maps Worker requests into the same
//! host-neutral example app use-cases used by native server and transport
//! adapters.

mod app_adapter;
mod entrypoint;
mod error;
mod model;
mod response;
mod routing;

#[cfg(test)]
mod tests;
