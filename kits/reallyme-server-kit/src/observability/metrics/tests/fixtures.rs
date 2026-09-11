// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use crate::config::{LogFormat, MetricsIdleTimeout, ObservabilityConfig, ServiceEnvironment};

pub(super) const SUBPROCESS_METRICS_TEST_ENV: &str = "REALLYME_SERVER_KIT_METRICS_SUBPROCESS";

pub(super) fn test_config() -> ObservabilityConfig {
    ObservabilityConfig::new(
        ServiceEnvironment::Local,
        LogFormat::Json,
        false,
        "reallyme_server_kit=info".to_owned(),
        MetricsIdleTimeout::new(Duration::from_secs(30)).expect("valid metrics timeout"),
    )
    .expect("valid observability config")
}
