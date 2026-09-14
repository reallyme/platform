// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

//! Reusable, security-hardened Valkey database connector.
//!
//! The kit owns environment and programmatic configuration, TLS-by-default
//! connection setup, reconnect policy, readiness reporting, namespaced binary
//! keys, generic typed commands and pipelines, TTL values, and low-cardinality
//! errors. App adapters own schema, serialization, and business semantics.

mod command;
mod config;
mod connector;
mod error;
mod value;

pub use command::{ValkeyCommand, ValkeyPipeline, valkey_command, valkey_pipeline};
pub use config::{ValkeyConfig, ValkeyConfigInput, ValkeyTlsTrust, ValkeyTransportSecurity};
pub use connector::{ValkeyConnector, ValkeyHealthReport, ValkeyProtocolVersion};
pub use error::{
    ValkeyCommandErrorReason, ValkeyConfigErrorReason, ValkeyConfigField, ValkeyDataErrorReason,
    ValkeyDataKind, ValkeyError, ValkeyResult, ValkeyRetryHint, ValkeySetupErrorReason,
};
pub use value::{ValkeyKey, ValkeyTimeToLive, ValkeyValue};
