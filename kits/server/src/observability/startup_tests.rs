// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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

    let summary = observability_startup_summary(&server_name, DeploymentRegion::default(), &config);

    assert_eq!(summary.server_name(), "reallyme-api");
    assert_eq!(summary.deployment_region().as_str(), "unknown");
    assert_eq!(summary.log_format(), LogFormat::Json);
    assert!(summary.tracing_filter_configured());
    assert!(!format!("{summary:?}").contains("reallyme_server_kit=info"));
}
