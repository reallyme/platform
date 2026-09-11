// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Construct and retain the hardened local-probe HTTP client.

use std::sync::OnceLock;

use reqwest::{Client, redirect};

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

static HTTP_CLIENT: OnceLock<AgentResult<Client>> = OnceLock::new();

/// Returns the process-wide client used only for local probe-style requests.
pub(crate) fn shared_http_client() -> AgentResult<&'static Client> {
    HTTP_CLIENT
        .get_or_init(build_http_client)
        .as_ref()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::TlsUnavailable))
}

fn build_http_client() -> AgentResult<Client> {
    Client::builder()
        // Probe endpoints must not redirect to a different host or protocol.
        .redirect(redirect::Policy::none())
        .build()
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::TlsUnavailable))
}
