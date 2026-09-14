// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app port and adapter naming conventions.

mod adapter;
mod descriptor;
mod downstream;
mod error;
mod health;
mod port_name;
mod retry;
mod timeout;

pub use adapter::AppAdapterKind;
pub use descriptor::AppPortDescriptor;
pub use downstream::AppDownstreamPortDescriptor;
pub use error::{AppPortError, AppPortErrorKind};
pub use health::AppPortHealth;
pub use port_name::AppPortName;
pub use retry::AppPortRetryPolicy;
pub use timeout::{AppPortTimeout, AppPortTimeoutError};
