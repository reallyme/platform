// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::config::{LogFormat, MetricsIdleTimeout, ObservabilityConfig};
use crate::startup::{DeploymentRegion, ServerName};

/// Safe startup summary for observability configuration.
///
/// This wrapper exists so services can emit one stable startup summary object
/// without reassembling observability fields ad hoc in each binary. The summary
/// intentionally includes only non-secret operational configuration. The raw
/// tracing filter is deliberately summarized as "configured" rather than
/// exposed because it originates from deployment configuration and may contain
/// arbitrary operator-provided module paths or directives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservabilityStartupSummary {
    server_name: String,
    deployment_region: DeploymentRegion,
    log_format: LogFormat,
    emit_span_events: bool,
    tracing_filter_configured: bool,
    metrics_idle_timeout: MetricsIdleTimeout,
}

impl ObservabilityStartupSummary {
    /// Returns the validated server name.
    pub fn server_name(&self) -> &str {
        self.server_name.as_str()
    }

    /// Returns the validated deployment region.
    pub fn deployment_region(&self) -> &DeploymentRegion {
        &self.deployment_region
    }

    /// Returns the configured log format.
    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }

    /// Returns whether span lifecycle events should be emitted.
    pub fn emit_span_events(&self) -> bool {
        self.emit_span_events
    }

    /// Returns whether a non-empty tracing filter was configured.
    pub fn tracing_filter_configured(&self) -> bool {
        self.tracing_filter_configured
    }

    /// Returns the metrics idle timeout.
    pub fn metrics_idle_timeout(&self) -> MetricsIdleTimeout {
        self.metrics_idle_timeout
    }
}

/// Builds a startup-safe observability summary.
pub fn observability_startup_summary(
    server_name: &ServerName,
    deployment_region: DeploymentRegion,
    config: &ObservabilityConfig,
) -> ObservabilityStartupSummary {
    ObservabilityStartupSummary {
        server_name: server_name.as_str().to_owned(),
        deployment_region,
        log_format: config.log_format(),
        emit_span_events: config.emit_span_events(),
        tracing_filter_configured: !config.tracing_filter_directives().trim().is_empty(),
        metrics_idle_timeout: config.metrics_idle_timeout(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::observability_startup_summary;
    use crate::config::{LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment};
    use crate::startup::{DeploymentRegion, ServerName};

    #[test]
    fn startup_summary_exposes_only_safe_observability_fields() {
        let server_name = ServerName::new("reallyme-api").expect("valid server name");
        let config = ObservabilityConfig::new(
            ServiceEnvironment::Prod,
            LogFormat::Json,
            true,
            "reallyme_server_kit=info".to_owned(),
            MetricsIdleTimeout::new(Duration::from_secs(60)).expect("valid metrics timeout"),
        )
        .expect("valid observability config");

        let summary =
            observability_startup_summary(&server_name, DeploymentRegion::default(), &config);

        assert_eq!(summary.server_name(), "reallyme-api");
        assert_eq!(summary.deployment_region().as_str(), "unknown");
        assert_eq!(summary.log_format(), LogFormat::Json);
        assert!(summary.tracing_filter_configured());
        assert!(!format!("{summary:?}").contains("reallyme_server_kit=info"));
    }
}
