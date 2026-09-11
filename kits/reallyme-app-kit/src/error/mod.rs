// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! App-kit validation and app error contract primitives.

mod contract;
mod field;
mod kit;

pub use contract::{AppErrorCategory, AppErrorCode, AppErrorContract, AppErrorRetryDisposition};
pub use field::AppKitField;
pub use kit::{AppKitError, AppKitErrorReason};
