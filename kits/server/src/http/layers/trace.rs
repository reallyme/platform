// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::extract::MatchedPath;
use axum::http::Request;
use axum::response::Response;
use pin_project_lite::pin_project;
use tower::{Layer, Service};
use tracing::Span;

use crate::config::{HttpRequestLogMode, HttpRequestLoggingConfig};
use crate::http::{ExternalRequestOrigin, ForwardedClientIp};
use crate::observability::{
    HttpMethodLabel, HttpStatusClass, MetricRouteTemplateLabel, http_request_span,
    record_http_request_outcome_for_route_template,
};

use super::super::ids::{request_id_from_headers, trace_id_from_headers};
use super::listener::listener_name_for_request;

/// Layer that emits transport-safe request completion logs.
#[derive(Debug, Clone, Copy)]
pub struct HttpTraceLayer {
    request_logging: HttpRequestLoggingConfig,
}

/// Creates the transport-level tracing layer.
///
/// This layer intentionally records only transport-safe request/response
/// fields: method, matched route template, stable IDs, HTTP status, and
/// latency. It never records bodies, auth headers, cookies, or raw query
/// strings.
pub fn trace_layer(request_logging: HttpRequestLoggingConfig) -> HttpTraceLayer {
    HttpTraceLayer { request_logging }
}

impl<S> Layer<S> for HttpTraceLayer {
    type Service = HttpTraceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        HttpTraceService {
            inner,
            request_logging: self.request_logging,
        }
    }
}

#[derive(Clone)]
pub struct HttpTraceService<S> {
    inner: S,
    request_logging: HttpRequestLoggingConfig,
}

impl<S> Service<Request<Body>> for HttpTraceService<S>
where
    S: Service<Request<Body>, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = HttpTraceResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let request_id = request_id_from_headers(request.headers());
        let trace_id = trace_id_from_headers(request.headers());
        let method = HttpMethodLabel::from_method(request.method());
        let matched_path = request.extensions().get::<MatchedPath>();
        let route_template = MetricRouteTemplateLabel::from_matched_path(matched_path);
        let listener_name = listener_name_for_request(&request).clone();
        let request_span = http_request_span(
            request.method(),
            matched_path.map(MatchedPath::as_str),
            request_id,
            trace_id,
        );
        record_selected_request_log_fields(
            &request_span,
            &request,
            self.request_logging,
            listener_name.as_str(),
        );
        let response_future = self.inner.call(request);

        HttpTraceResponseFuture::new(
            response_future,
            request_span,
            Instant::now(),
            method,
            route_template,
            listener_name,
            self.request_logging,
        )
    }
}

pin_project! {
    /// Response future for [`HttpTraceService`].
    pub struct HttpTraceResponseFuture<F> {
        #[pin]
        inner: F,
        span: Span,
        started_at: Instant,
        method: HttpMethodLabel,
        route_template: MetricRouteTemplateLabel,
        listener_name: crate::http::HttpListenerName,
        request_logging: HttpRequestLoggingConfig,
    }
}

impl<F> HttpTraceResponseFuture<F> {
    fn new(
        inner: F,
        span: Span,
        started_at: Instant,
        method: HttpMethodLabel,
        route_template: MetricRouteTemplateLabel,
        listener_name: crate::http::HttpListenerName,
        request_logging: HttpRequestLoggingConfig,
    ) -> Self {
        Self {
            inner,
            span,
            started_at,
            method,
            route_template,
            listener_name,
            request_logging,
        }
    }
}

impl<F> Future for HttpTraceResponseFuture<F>
where
    F: Future<Output = Result<Response, Infallible>>,
{
    type Output = Result<Response, Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let _guard = this.span.enter();
        let response = ready!(this.inner.poll(cx))?;
        let latency = this.started_at.elapsed();
        let latency_ms = duration_millis_saturating_u64(latency);
        let status_class = HttpStatusClass::from_status_code(response.status().as_u16());

        record_http_request_outcome_for_route_template(
            this.listener_name,
            *this.method,
            this.route_template,
            status_class,
            latency,
        );
        let slow_threshold = this.request_logging.slow_request_threshold();
        let is_slow = slow_threshold.is_some_and(|threshold| latency >= threshold);
        let should_log = should_log_request_completion(
            *this.request_logging,
            status_class,
            this.method,
            this.route_template.as_str(),
            latency,
            is_slow,
        );
        if should_log {
            let status_code = response.status().as_u16();
            if status_code >= 500 || is_slow {
                tracing::warn!(
                    http.status_code = status_code,
                    http.status_class = status_class.as_str(),
                    http.latency_ms = latency_ms,
                    http.slow_request = is_slow,
                    "http request completed"
                );
            } else if status_code >= 400 {
                tracing::debug!(
                    http.status_code = status_code,
                    http.status_class = status_class.as_str(),
                    http.latency_ms = latency_ms,
                    http.slow_request = is_slow,
                    "http request completed"
                );
            } else {
                tracing::info!(
                    http.status_code = status_code,
                    http.status_class = status_class.as_str(),
                    http.latency_ms = latency_ms,
                    http.slow_request = is_slow,
                    "http request completed"
                );
            }
        }

        Poll::Ready(Ok(response))
    }
}

fn should_log_request_completion(
    config: HttpRequestLoggingConfig,
    status_class: HttpStatusClass,
    method: &HttpMethodLabel,
    route_template: &str,
    latency: Duration,
    is_slow: bool,
) -> bool {
    if is_slow || matches!(status_class, HttpStatusClass::ServerError) {
        return true;
    }

    if matches!(config.mode(), HttpRequestLogMode::ErrorsOnly) {
        return matches!(status_class, HttpStatusClass::ClientError);
    }

    match config.mode() {
        HttpRequestLogMode::Disabled => false,
        HttpRequestLogMode::ErrorsOnly => false,
        HttpRequestLogMode::All => true,
        HttpRequestLogMode::Sampled => {
            if matches!(status_class, HttpStatusClass::ClientError) {
                return false;
            }
            should_sample_success(
                method.as_str(),
                route_template,
                latency.as_nanos(),
                config.sample_rate(),
            )
        }
    }
}

fn should_sample_success(
    method: &str,
    route_template: &str,
    latency_nanos: u128,
    sample_rate: f64,
) -> bool {
    const SAMPLING_HASH_BITS: u32 = 53;
    const SAMPLING_HASH_SPACE: u64 = 1_u64 << SAMPLING_HASH_BITS;
    const SAMPLING_HASH_MASK: u64 = SAMPLING_HASH_SPACE - 1;

    if sample_rate <= 0.0 {
        return false;
    }
    if sample_rate >= 1.0 {
        return true;
    }

    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    method.hash(&mut hasher);
    route_template.hash(&mut hasher);
    latency_nanos.hash(&mut hasher);

    // Use an integer comparison over an exactly representable 53-bit bucket
    // space instead of normalizing the full u64 into f64. This keeps the
    // boundary decision stable and avoids the high-end rounding bias that can
    // appear when large u64 values are converted directly to floating point.
    let sample_bucket = hasher.finish() & SAMPLING_HASH_MASK;
    let threshold = (sample_rate * (SAMPLING_HASH_SPACE as f64)).floor() as u64;
    sample_bucket < threshold
}

fn duration_millis_saturating_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn record_selected_request_log_fields(
    span: &Span,
    request: &Request<Body>,
    config: HttpRequestLoggingConfig,
    listener_name: &str,
) {
    let fields = config.fields();

    if fields.listener_name() {
        span.record("listener.name", tracing::field::display(listener_name));
    }

    if fields.normalized_client_ip()
        && let Some(client_ip) = request
            .extensions()
            .get::<ForwardedClientIp>()
            .map(|client_ip| (*client_ip).into_ip_addr())
            .or_else(|| {
                request
                    .extensions()
                    .get::<ConnectInfo<SocketAddr>>()
                    .map(|connect_info| connect_info.0.ip())
            })
    {
        span.record("client.ip", tracing::field::display(client_ip));
    }

    let Some(external_origin) = request.extensions().get::<ExternalRequestOrigin>() else {
        return;
    };

    if fields.external_host() {
        span.record(
            "http.external_host",
            tracing::field::display(external_origin.host().host()),
        );
    }

    if fields.external_proto() {
        span.record(
            "http.external_proto",
            tracing::field::display(external_origin.proto().as_str()),
        );
    }

    if fields.external_port()
        && let Some(port) = external_origin.port()
    {
        span.record("http.external_port", tracing::field::display(port.as_u16()));
    }

    if fields.external_origin() {
        span.record(
            "http.external_origin",
            tracing::field::display(external_origin.base_url()),
        );
    }
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
