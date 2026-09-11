// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! node-exporter metrics observation.

use std::time::Duration;

use url::Url;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::http_client::shared_http_client;

const MAX_NODE_EXPORTER_METRICS_BYTES: usize = 2 * 1024 * 1024;

/// Minimal node-exporter signal used by the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeExporterSnapshot {
    metric_lines: u32,
    has_cpu_metrics: bool,
    has_memory_metrics: bool,
    has_filesystem_metrics: bool,
}

impl NodeExporterSnapshot {
    /// Returns the number of metric sample lines.
    pub const fn metric_lines(self) -> u32 {
        self.metric_lines
    }

    /// Returns whether CPU metrics were observed.
    pub const fn has_cpu_metrics(self) -> bool {
        self.has_cpu_metrics
    }

    /// Returns whether memory metrics were observed.
    pub const fn has_memory_metrics(self) -> bool {
        self.has_memory_metrics
    }

    /// Returns whether filesystem metrics were observed.
    pub const fn has_filesystem_metrics(self) -> bool {
        self.has_filesystem_metrics
    }
}

/// Fetches and validates the local node-exporter Prometheus endpoint.
pub async fn collect_node_exporter_metrics(
    url: &Url,
    request_timeout: Duration,
) -> AgentResult<NodeExporterSnapshot> {
    let mut response = shared_http_client()?
        .get(url.as_str())
        .timeout(request_timeout)
        .send()
        .await
        .map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::MetricsEndpointUnavailable)
        })?;
    if !response.status().is_success() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::MetricsEndpointUnavailable,
        ));
    }
    let body = read_limited_metrics_body(
        &mut response,
        MAX_NODE_EXPORTER_METRICS_BYTES,
        HephaestusAgentErrorReason::MetricsEndpointUnavailable,
    )
    .await?;
    let text = std::str::from_utf8(body.as_slice()).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::MetricsEndpointUnavailable)
    })?;
    Ok(parse_node_exporter_metrics(text))
}

async fn read_limited_metrics_body(
    response: &mut reqwest::Response,
    max_bytes: usize,
    reason: HephaestusAgentErrorReason,
) -> AgentResult<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_error| HephaestusAgentError::new(reason))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| HephaestusAgentError::new(reason))?;
        if next_len > max_bytes {
            return Err(HephaestusAgentError::new(reason));
        }
        body.extend_from_slice(chunk.as_ref());
    }
    Ok(body)
}

fn parse_node_exporter_metrics(text: &str) -> NodeExporterSnapshot {
    let mut metric_lines = 0u32;
    let mut has_cpu_metrics = false;
    let mut has_memory_metrics = false;
    let mut has_filesystem_metrics = false;
    for line in text.lines() {
        if !line.starts_with("node_") {
            continue;
        }
        metric_lines = metric_lines.saturating_add(1);
        if !has_cpu_metrics && line.starts_with("node_cpu_seconds_total") {
            has_cpu_metrics = true;
        }
        if !has_memory_metrics && line.starts_with("node_memory_MemTotal_bytes") {
            has_memory_metrics = true;
        }
        if !has_filesystem_metrics && line.starts_with("node_filesystem_size_bytes") {
            has_filesystem_metrics = true;
        }
    }
    NodeExporterSnapshot {
        metric_lines,
        has_cpu_metrics,
        has_memory_metrics,
        has_filesystem_metrics,
    }
}

#[cfg(test)]
mod tests;
