// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

//! Shared NATS and JetStream infrastructure primitives.
//!
//! This crate owns validated subject handling, JetStream connection,
//! configuration, publishing, consumption, acknowledgment,
//! deterministic dedupe/message-id helpers, and test-fake mechanics.
//! Apps remain responsible for their payload definitions and business logic.

/// Configuration and typed connection helpers.
pub mod config;
/// Consumer backends and delivery wrappers.
pub mod consumer;
/// Shared error reasons and typed transport failure types.
pub mod error;
/// Deterministic message-id helpers for JetStream dedupe.
pub mod message_id;
/// Publisher backends and ack helpers.
pub mod publisher;
/// Test fakes and in-memory fixtures for local verification.
pub mod testing;
