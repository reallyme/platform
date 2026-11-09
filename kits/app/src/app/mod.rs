// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generic host-neutral app scaffolding.

mod context;
mod core;
mod state;

pub use context::StandardAppContext;
pub use core::StandardAppCore;
pub use state::StandardAppState;
