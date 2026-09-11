// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

//! Hephaestus agent binary entrypoint.

use std::path::PathBuf;

use hephaestus_agent::runtime;

const DEFAULT_CONFIG_PATH: &str = "/etc/reallyme/hephaestus-agent/config.toml";

#[tokio::main]
async fn main() {
    init_tracing();
    let config_path = config_path_from_args();
    if let Err(error) = runtime::run(config_path.as_path()).await {
        tracing::error!(reason = ?error.reason(), "hephaestus agent stopped with an error");
        std::process::exit(1);
    }
}

fn init_tracing() {
    // Logging stays in machine-readable JSON for journald and structured ingestion.
    // Increase human readability locally with `RUST_LOG` plus a jq formatter in pipelines.
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .finish();
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        eprintln!("hephaestus-agent failed to install tracing subscriber");
        std::process::exit(1);
    }
}

fn config_path_from_args() -> PathBuf {
    let mut args = std::env::args().skip(1);
    let mut selected = PathBuf::from(DEFAULT_CONFIG_PATH);
    while let Some(arg) = args.next() {
        if arg == "--config"
            && let Some(path) = args.next()
        {
            selected = PathBuf::from(path);
        }
    }
    selected
}
