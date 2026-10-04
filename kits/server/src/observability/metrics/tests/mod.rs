// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Metrics test modules.

#[cfg(feature = "http")]
mod exporter;
#[cfg(feature = "http")]
mod fixtures;
mod labels;
