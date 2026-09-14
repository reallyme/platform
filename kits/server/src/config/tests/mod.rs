// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Focused tests for the configuration module.

#[cfg(feature = "tonic-grpc")]
mod fixtures;
#[cfg(feature = "tonic-grpc")]
mod subprocess;
mod unit;
