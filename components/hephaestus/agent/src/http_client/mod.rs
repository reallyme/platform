// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Shared outbound HTTP client for local probe-style requests.

mod client;

pub(crate) use client::shared_http_client;

#[cfg(test)]
mod tests;
