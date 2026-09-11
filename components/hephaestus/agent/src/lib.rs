// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Node-local Hephaestus actuator and reporter.

pub mod actions;
pub mod audit;
pub mod cadvisor;
mod command;
pub mod config;
pub mod control_plane;
pub mod docker;
pub mod docker_engine;
pub mod docker_service;
pub mod error;
pub mod foundationdb_service;
pub mod host;
mod http_client;
pub mod identity;
pub mod node_exporter;
pub mod report;
pub mod runtime;
pub mod service_probe;
pub mod state;
pub mod systemd;
pub mod tailscale;

pub use error::{HephaestusAgentError, HephaestusAgentErrorReason};
