// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Process shutdown signal primitives.
//!
//! # Examples
//!
//! ```rust
//! use std::time::Duration;
//!
//! use reallyme_server_kit::shutdown::{ShutdownReason, shutdown_signal};
//!
//! let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
//!
//! runtime.block_on(async {
//!     let result = tokio::time::timeout(Duration::from_millis(1), shutdown_signal()).await;
//!
//!     assert!(result.is_err(), "doctests should not receive a real shutdown signal");
//!     let _ = ShutdownReason::CtrlC;
//! });
//! ```

mod error;
mod policy;
mod reason;
mod signal;

pub use crate::task::{BackgroundTaskSet, ShutdownController, ShutdownTimeout, ShutdownToken};
pub use error::{
    ShutdownError, ShutdownSignalKind, ShutdownValidationErrorReason, TaskJoinFailureReason,
};
pub use policy::{ShutdownMode, ShutdownModeParseError, ShutdownPolicy};
pub use reason::ShutdownReason;
pub use signal::shutdown_signal;
