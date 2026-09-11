// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use metrics_util::MetricKindMask;

use crate::config::ObservabilityConfig;

use super::super::error::ObservabilityError;
use super::describe::describe_standard_metrics;

/// Render helper returned after Prometheus recorder installation.
#[derive(Clone)]
pub struct MetricsExporter {
    handle: PrometheusHandle,
}

/// Prometheus exposure model used by server-kit.
///
/// The runtime intentionally exposes metrics by rendering a handle from the
/// standard `/metrics` operational route. It does not run a separate embedded
/// Prometheus HTTP server, which keeps listener ownership and access policy in
/// one audited server-runtime path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrometheusExposureModel {
    /// Metrics are rendered through the runtime-owned operational route.
    RenderHandleOperationalRoute,
}

impl MetricsExporter {
    /// Renders the current Prometheus exposition payload.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::time::Duration;
    ///
    /// use reallyme_server_kit::config::{LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment};
    /// use reallyme_server_kit::observability::{install_prometheus_recorder, record_readiness_state};
    /// use reallyme_server_kit::health::ReadinessState;
    ///
    /// let config = ObservabilityConfig::new(
    ///     ServiceEnvironment::Local,
    ///     LogFormat::PlainText,
    ///     false,
    ///     "reallyme_server_kit=info".to_owned(),
    ///     MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
    /// )
    /// .expect("valid observability config");
    ///
    /// let exporter = install_prometheus_recorder(&config).expect("metrics should initialize");
    /// record_readiness_state(ReadinessState::NotReady);
    ///
    /// let rendered = exporter.render();
    /// assert!(rendered.contains("reallyme_service_readiness_state"));
    /// ```
    pub fn render(&self) -> String {
        self.handle.render()
    }

    /// Returns how this exporter is exposed.
    pub const fn exposure_model(&self) -> PrometheusExposureModel {
        let _handle = &self.handle;

        PrometheusExposureModel::RenderHandleOperationalRoute
    }
}

/// Installs the process-global Prometheus recorder and returns a handle that
/// can render the current metrics snapshot.
pub fn install_prometheus_recorder(
    config: &ObservabilityConfig,
) -> Result<MetricsExporter, ObservabilityError> {
    let handle = PrometheusBuilder::new()
        .idle_timeout(
            MetricKindMask::ALL,
            Some(config.metrics_idle_timeout().as_duration()),
        )
        .install_recorder()
        .map_err(|_| ObservabilityError::MetricsRecorderAlreadyInstalled)?;

    describe_standard_metrics();

    Ok(MetricsExporter { handle })
}
