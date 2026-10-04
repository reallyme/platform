// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{TypesenseConfig, TypesenseConfigError, TypesenseConfigErrorReason};

const MAX_ENDPOINTS: usize = 8;
const MAX_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_IMPORT_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_RETRIES: u8 = 8;
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
const MAX_CONNECTION_INTERVAL: Duration = Duration::from_secs(600);

pub(super) fn validate_config(config: &TypesenseConfig) -> Result<(), TypesenseConfigError> {
    if config.endpoints.is_empty() || config.endpoints.len() > MAX_ENDPOINTS {
        return Err(invalid(TypesenseConfigErrorReason::InvalidEndpointCount));
    }
    if !valid_duration(config.request_timeout, MAX_REQUEST_TIMEOUT)
        || config
            .import_request_timeout
            .is_some_and(|value| !valid_duration(value, MAX_IMPORT_TIMEOUT))
    {
        return Err(invalid(TypesenseConfigErrorReason::InvalidRequestDeadline));
    }
    if config.max_retries > MAX_RETRIES
        || !valid_duration(config.retry_initial_delay, MAX_RETRY_DELAY)
        || !valid_duration(config.retry_max_delay, MAX_RETRY_DELAY)
        || config.retry_initial_delay > config.retry_max_delay
        || config.retry_jitter_percent > 100
    {
        return Err(invalid(TypesenseConfigErrorReason::InvalidRetryPolicy));
    }
    if config
        .pool_idle_timeout
        .is_some_and(|value| !valid_duration(value, MAX_CONNECTION_INTERVAL))
        || config
            .tcp_keepalive
            .is_some_and(|value| !valid_duration(value, MAX_CONNECTION_INTERVAL))
        || config
            .http2_keep_alive_interval
            .is_some_and(|value| !valid_duration(value, MAX_CONNECTION_INTERVAL))
    {
        return Err(invalid(TypesenseConfigErrorReason::InvalidConnectionTiming));
    }
    Ok(())
}

fn valid_duration(value: Duration, maximum: Duration) -> bool {
    !value.is_zero() && value <= maximum
}

const fn invalid(reason: TypesenseConfigErrorReason) -> TypesenseConfigError {
    TypesenseConfigError::Invalid { reason }
}
