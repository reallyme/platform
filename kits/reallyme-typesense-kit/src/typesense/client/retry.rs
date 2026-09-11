// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    DEFAULT_RETRY_ENTROPY_SEED, LABEL_OPERATION, LABEL_OUTCOME, METRIC_REQUEST_DURATION_SECONDS,
    METRIC_REQUEST_ERRORS_TOTAL, METRIC_REQUEST_RETRIES_TOTAL, METRIC_REQUESTS_IN_FLIGHT,
    METRIC_REQUESTS_TOTAL, RequestKind, TypesenseClient, advance_retry_entropy,
};
use crate::typesense::{
    TypesenseEndpoint, TypesenseEndpointSelection, TypesenseError, TypesenseTransportReason,
    TypesenseUpstreamReason,
    error::{TypesenseResult, map_status},
};
use metrics::{counter, gauge, histogram};
use reqwest::{
    Response, StatusCode,
    header::{HeaderMap, RETRY_AFTER},
};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::Instrument;

impl TypesenseClient {
    pub(super) async fn execute_with_retries(
        &self,
        request_timeout: Duration,
        mut build_request: impl FnMut(&TypesenseEndpoint) -> reqwest::RequestBuilder,
        request_kind: RequestKind,
    ) -> TypesenseResult<Response> {
        struct InFlightRequestGuard {
            operation_label: &'static str,
        }

        impl Drop for InFlightRequestGuard {
            fn drop(&mut self) {
                gauge!(METRIC_REQUESTS_IN_FLIGHT, LABEL_OPERATION => self.operation_label)
                    .decrement(1.0);
            }
        }

        let operation_label = request_kind.as_str();
        let span = tracing::info_span!("typesense.request", operation = operation_label);
        async move {
            let started_at = Instant::now();
            gauge!(METRIC_REQUESTS_IN_FLIGHT, LABEL_OPERATION => operation_label).increment(1.0);
            let _in_flight_guard = InFlightRequestGuard { operation_label };
            counter!(METRIC_REQUESTS_TOTAL, LABEL_OPERATION => operation_label).increment(1);

            if self.endpoints.is_empty() {
                counter!(
                    METRIC_REQUEST_ERRORS_TOTAL,
                    LABEL_OPERATION => operation_label,
                    LABEL_OUTCOME => "transport"
                )
                .increment(1);
                return Err(TypesenseError::Transport {
                    reason: TypesenseTransportReason::RequestFailed,
                });
            }
            let start_index = self.start_endpoint_index(self.endpoints.len());

            for attempt in 0..=self.max_retries {
                let endpoint_index = self
                    .endpoint_index(start_index, attempt)
                    .wrapping_rem(self.endpoints.len());
                let endpoint =
                    &self
                        .endpoints
                        .get(endpoint_index)
                        .ok_or(TypesenseError::Transport {
                            reason: TypesenseTransportReason::RequestFailed,
                        })?;

                let response = build_request(endpoint)
                    .timeout(request_timeout)
                    .send()
                    .await;

                match response {
                    Ok(response) => {
                        let status = response.status();
                        if status.is_success() {
                            histogram!(
                                METRIC_REQUEST_DURATION_SECONDS,
                                LABEL_OPERATION => operation_label,
                                LABEL_OUTCOME => "success"
                            )
                            .record(started_at.elapsed().as_secs_f64());
                            return Ok(response);
                        }

                        let retry_after = self.parse_retry_after(response.headers(), status);
                        // A server's minimum wait cannot be shortened to fit our
                        // local budget. Return the upstream error without retrying.
                        if !self.should_retry_status(status)
                            || attempt == self.max_retries
                            || retry_after.is_some_and(|delay| delay > self.retry_max_delay)
                        {
                            histogram!(
                                METRIC_REQUEST_DURATION_SECONDS,
                                LABEL_OPERATION => operation_label,
                                LABEL_OUTCOME => "error"
                            )
                            .record(started_at.elapsed().as_secs_f64());
                            counter!(
                                METRIC_REQUEST_ERRORS_TOTAL,
                                LABEL_OPERATION => operation_label,
                                LABEL_OUTCOME => "upstream"
                            )
                            .increment(1);
                            return Err(self.map_status(status, request_kind));
                        }

                        counter!(METRIC_REQUEST_RETRIES_TOTAL, LABEL_OPERATION => operation_label)
                            .increment(1);
                        self.sleep_backoff(attempt, retry_after).await;
                    }
                    Err(_) => {
                        if attempt == self.max_retries {
                            histogram!(
                                METRIC_REQUEST_DURATION_SECONDS,
                                LABEL_OPERATION => operation_label,
                                LABEL_OUTCOME => "error"
                            )
                            .record(started_at.elapsed().as_secs_f64());
                            counter!(
                                METRIC_REQUEST_ERRORS_TOTAL,
                                LABEL_OPERATION => operation_label,
                                LABEL_OUTCOME => "transport"
                            )
                            .increment(1);
                            return Err(TypesenseError::Transport {
                                reason: TypesenseTransportReason::RequestFailed,
                            });
                        }

                        counter!(METRIC_REQUEST_RETRIES_TOTAL, LABEL_OPERATION => operation_label)
                            .increment(1);
                        self.sleep_backoff(attempt, None).await;
                    }
                }
            }

            histogram!(
                METRIC_REQUEST_DURATION_SECONDS,
                LABEL_OPERATION => operation_label,
                LABEL_OUTCOME => "error"
            )
            .record(started_at.elapsed().as_secs_f64());
            counter!(
            METRIC_REQUEST_ERRORS_TOTAL,
            LABEL_OPERATION => operation_label,
                LABEL_OUTCOME => "transport"
            )
            .increment(1);
            Err(TypesenseError::Transport {
                reason: TypesenseTransportReason::RequestFailed,
            })
        }
        .instrument(span)
        .await
    }

    fn map_status(&self, status: StatusCode, request_kind: RequestKind) -> TypesenseError {
        if status == StatusCode::NOT_FOUND
            && matches!(
                request_kind,
                RequestKind::GetDocument | RequestKind::DeleteDocument
            )
        {
            return TypesenseError::Upstream {
                reason: TypesenseUpstreamReason::DocumentNotFound,
                status,
            };
        }

        map_status(status)
    }

    fn should_retry_status(&self, status: StatusCode) -> bool {
        status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
    }

    fn parse_retry_after(&self, headers: &HeaderMap, status: StatusCode) -> Option<Duration> {
        if status != StatusCode::TOO_MANY_REQUESTS {
            return None;
        }

        headers
            .get(RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_secs)
            .filter(|value| !value.is_zero())
    }

    async fn sleep_backoff(&self, attempt: u8, retry_after: Option<Duration>) {
        let delay = self.calculate_backoff_delay(attempt, retry_after);
        if !delay.is_zero() {
            sleep(delay).await;
        }
    }

    fn calculate_backoff_delay(&self, attempt: u8, retry_after: Option<Duration>) -> Duration {
        let mut delay = self.retry_initial_delay;
        for _ in 0..attempt {
            delay = delay.checked_mul(2).unwrap_or(self.retry_max_delay);
            if delay > self.retry_max_delay {
                delay = self.retry_max_delay;
                break;
            }
        }

        if let Some(retry_after_delay) = retry_after
            && retry_after_delay > delay
        {
            delay = retry_after_delay;
        }

        if delay > self.retry_max_delay {
            delay = self.retry_max_delay;
        }

        if retry_after.is_some() {
            return delay;
        }

        let jitter_percent = self.retry_jitter_percent.min(100);
        if jitter_percent == 0 {
            return delay;
        }

        let delay_nanos = delay.as_nanos();
        let jitter_nanos = delay_nanos.saturating_mul(u128::from(jitter_percent)) / 100u128;
        if jitter_nanos == 0 {
            return delay;
        }

        let jitter_max_nanos = u64::try_from(jitter_nanos).unwrap_or(u64::MAX);
        let jitter = if jitter_max_nanos == 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(self.next_jitter_nanos(jitter_max_nanos))
        };

        // Move the jitter window below the ceiling when the exponential delay
        // reaches it; clipping every positive sample would synchronize retries.
        let latest_base = self
            .retry_max_delay
            .checked_sub(Duration::from_nanos(jitter_max_nanos))
            .unwrap_or(Duration::ZERO);
        delay
            .min(latest_base)
            .checked_add(jitter)
            .unwrap_or(self.retry_max_delay)
    }

    fn next_jitter_nanos(&self, max_nanos: u64) -> u64 {
        let mut observed = self.retry_entropy.load(Ordering::Relaxed);

        loop {
            let current = if observed == 0 {
                DEFAULT_RETRY_ENTROPY_SEED
            } else {
                observed
            };
            let next = advance_retry_entropy(current);
            match self.retry_entropy.compare_exchange_weak(
                observed,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return next % max_nanos,
                Err(actual) => observed = actual,
            }
        }
    }

    pub(super) fn start_endpoint_index(&self, endpoint_count: usize) -> usize {
        match self.endpoint_selection {
            TypesenseEndpointSelection::NearestNode => 0,
            TypesenseEndpointSelection::RoundRobin | TypesenseEndpointSelection::Failover => self
                .endpoint_cursor
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_rem(endpoint_count),
        }
    }

    pub(super) fn endpoint_index(&self, start_index: usize, attempt: u8) -> usize {
        match self.endpoint_selection {
            TypesenseEndpointSelection::RoundRobin => start_index,
            TypesenseEndpointSelection::NearestNode | TypesenseEndpointSelection::Failover => {
                start_index.wrapping_add(usize::from(attempt))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::TypesenseClient;
    use crate::typesense::{TypesenseConfig, TypesenseEndpoint};
    use secrecy::SecretString;
    use std::time::Duration;

    #[test]
    fn server_retry_after_and_jitter_cannot_exceed_local_policy() {
        let config = TypesenseConfig::new(
            TypesenseEndpoint::parse("https://typesense.invalid").expect("endpoint"),
            SecretString::from("key"),
            Duration::from_secs(1),
        )
        .expect("config")
        .with_retry_max_delay(Duration::from_secs(2))
        .with_retry_initial_delay(Duration::from_secs(2))
        .with_retry_jitter_percent(100);
        let client = TypesenseClient::new(reqwest::Client::new(), config);
        let first = client.calculate_backoff_delay(32, None);
        assert!((0..32).any(|_| client.calculate_backoff_delay(32, None) != first));
        for attempt in [0, 1, 32, 255] {
            assert!(
                client.calculate_backoff_delay(attempt, Some(Duration::MAX))
                    <= Duration::from_secs(2)
            );
            assert!(client.calculate_backoff_delay(attempt, None) <= Duration::from_secs(2));
        }
    }
}
