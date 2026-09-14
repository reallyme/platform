// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host and transport adapters for the example app.

#[cfg(feature = "connect")]
pub mod connect;
#[cfg(feature = "http")]
pub mod http;
pub mod in_process;
#[cfg(feature = "native-server")]
pub mod server;
