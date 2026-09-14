// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::observability::LogFormat;

/// Default request timeout used by generic HTTP middleware if a service chooses
/// to adopt the server-kit baseline.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Default request body limit used by generic HTTP middleware if a service
/// chooses to adopt the server-kit baseline.
pub const DEFAULT_REQUEST_BODY_LIMIT_BYTES: usize = 1_048_576;
/// Default metrics idle timeout used by the server-kit Prometheus recorder if
/// a service chooses to adopt the baseline configuration.
pub const DEFAULT_METRICS_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Default log format recommended by the server kit for deployed services.
pub const DEFAULT_LOG_FORMAT: LogFormat = LogFormat::Json;
/// Default span-event behavior for tracing.
pub const DEFAULT_EMIT_SPAN_EVENTS: bool = false;
