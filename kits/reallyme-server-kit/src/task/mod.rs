// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Managed background task lifecycle helpers.
//!
//! This module owns the reusable primitives for spawning, supervising, and
//! draining long-lived Tokio tasks under explicit shutdown control. Server
//! processes and runtime apps should prefer these helpers over scattered ad hoc
//! `tokio::spawn` calls so lifecycle ownership remains centralized and
//! reviewable.
//!
//! # Examples
//!
//! ```rust
//! use std::time::Duration;
//!
//! use reallyme_server_kit::shutdown::ShutdownReason;
//! use reallyme_server_kit::startup::TaskName;
//! use reallyme_server_kit::task::{BackgroundTaskSet, ShutdownTimeout};
//!
//! let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
//!
//! runtime.block_on(async {
//!     let mut tasks = BackgroundTaskSet::new();
//!
//!     tasks
//!         .spawn(
//!             TaskName::new("index-refresher").expect("valid task name"),
//!             |mut shutdown| async move {
//!                 let _ = shutdown.cancelled().await;
//!             },
//!         )
//!         .expect("task should register");
//!
//!     let timeout = ShutdownTimeout::new(Duration::from_secs(1)).expect("valid shutdown timeout");
//!     tasks
//!         .shutdown(ShutdownReason::Sigterm, timeout)
//!         .await
//!         .expect("task should shut down cleanly");
//! });
//! ```

mod channel;
mod error;
mod shutdown;
mod spawn;
mod supervisor;

pub use channel::{TaskChannelCapacity, bounded_task_channel, try_send_bounded_task};
pub use error::{
    TaskChannelError, TaskChannelValidationErrorReason, TaskExecutionError, TaskExecutionErrorKind,
    TaskSetError, TaskSetValidationErrorReason,
};
pub use shutdown::{ShutdownController, ShutdownTimeout, ShutdownToken};
pub use supervisor::{
    BackgroundTaskSet, DEFAULT_BACKGROUND_TASK_CAPACITY, MAX_BACKGROUND_TASK_CAPACITY,
    TaskSetCapacity,
};
