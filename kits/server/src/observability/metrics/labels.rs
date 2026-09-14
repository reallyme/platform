// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "http")]
use std::collections::HashSet;
#[cfg(feature = "http")]
use std::sync::{Arc, LazyLock, RwLock};

#[cfg(feature = "http")]
use axum::extract::MatchedPath;
#[cfg(feature = "http")]
use axum::http::Method;
#[cfg(feature = "http")]
use metrics::SharedString;

use super::super::error::MetricLabelError;

#[cfg(feature = "http")]
const MAX_RETAINED_ROUTE_TEMPLATE_LABELS: usize = 4_096;

#[cfg(feature = "http")]
static RETAINED_ROUTE_TEMPLATE_LABELS: LazyLock<RwLock<HashSet<Arc<str>>>> =
    LazyLock::new(|| RwLock::new(HashSet::new()));

pub(crate) const METRIC_LABEL_METHOD: &str = "method";
pub(crate) const METRIC_LABEL_LISTENER_NAME: &str = "listener_name";
pub(crate) const METRIC_LABEL_ROUTE: &str = "route";
#[cfg(feature = "http")]
pub(crate) const METRIC_LABEL_REJECTION_REASON: &str = "reason";
#[cfg(feature = "http")]
pub(crate) const METRIC_LABEL_TRANSPORT: &str = "transport";
#[cfg(feature = "http")]
pub(crate) const METRIC_LABEL_RATE_LIMIT_TIER: &str = "rate_limit_tier";
#[cfg(feature = "http")]
pub(crate) const METRIC_LABEL_RATE_LIMIT_OUTCOME: &str = "rate_limit_outcome";
pub(crate) const METRIC_LABEL_RUNTIME_QUEUE: &str = "queue";
pub(crate) const METRIC_LABEL_RUNTIME_OUTCOME: &str = "outcome";
#[cfg(feature = "http")]
pub(crate) const METRIC_LABEL_DISCOVERY_SOURCE: &str = "source";
#[cfg(feature = "http")]
pub(crate) const METRIC_LABEL_DISCOVERY_OUTCOME: &str = "outcome";
pub(crate) const METRIC_LABEL_STATUS_CLASS: &str = "status_class";
pub(crate) const METRIC_LABEL_SERVER_NAME: &str = "server_name";
pub(crate) const METRIC_LABEL_SERVICE_VERSION: &str = "service_version";
pub(crate) const METRIC_LABEL_GIT_SHA: &str = "git_sha";

/// Low-cardinality route label used when Axum has no matched route template.
pub const UNKNOWN_ROUTE_TEMPLATE: &str = "/__unmatched";
/// Low-cardinality listener label used outside a named server runtime listener.
pub const UNKNOWN_LISTENER_NAME: &str = "__unknown";

/// Retained low-cardinality route label for request-path metrics.
///
/// Metrics keys outlive the request, so non-static route templates must be
/// retained before they are emitted. This type keeps that retention explicit
/// and bounded: callers may construct it only from Axum matched route
/// templates, configured gRPC method paths, static constants, or the standard
/// unknown-route sentinel. Raw URI paths must never be interned here.
#[cfg(feature = "http")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetricRouteTemplateLabel(SharedString);

#[cfg(feature = "http")]
impl MetricRouteTemplateLabel {
    pub(crate) fn unknown() -> Self {
        Self(UNKNOWN_ROUTE_TEMPLATE.into())
    }

    pub(crate) fn from_matched_path(matched_path: Option<&MatchedPath>) -> Self {
        match matched_path {
            Some(matched_path) => Self(retain_route_template_label(matched_path.as_str())),
            None => Self::unknown(),
        }
    }

    #[cfg(feature = "tonic-grpc")]
    pub(crate) fn from_runtime_route_template(value: &str) -> Self {
        Self(retain_route_template_label(value))
    }

    pub(crate) fn clone_shared(&self) -> SharedString {
        self.0.clone()
    }

    pub(crate) fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

#[cfg(feature = "http")]
fn retain_route_template_label(value: &str) -> SharedString {
    if value == UNKNOWN_ROUTE_TEMPLATE {
        return UNKNOWN_ROUTE_TEMPLATE.into();
    }

    if !route_template_value_is_metric_safe(value) {
        return UNKNOWN_ROUTE_TEMPLATE.into();
    }

    let existing = match RETAINED_ROUTE_TEMPLATE_LABELS.read() {
        Ok(guard) => guard.get(value).cloned(),
        Err(poisoned) => poisoned.into_inner().get(value).cloned(),
    };
    if let Some(existing) = existing {
        return SharedString::from_shared(existing);
    }

    let mut guard = match RETAINED_ROUTE_TEMPLATE_LABELS.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(existing) = guard.get(value).cloned() {
        return SharedString::from_shared(existing);
    }
    if guard.len() >= MAX_RETAINED_ROUTE_TEMPLATE_LABELS {
        return UNKNOWN_ROUTE_TEMPLATE.into();
    }

    let retained = Arc::<str>::from(value);
    guard.insert(Arc::clone(&retained));
    SharedString::from_shared(retained)
}

#[cfg(feature = "http")]
fn route_template_value_is_metric_safe(value: &str) -> bool {
    !value.is_empty()
        && value.starts_with('/')
        && !value.contains('?')
        && !value.contains("://")
        && !value.chars().any(char::is_whitespace)
}

/// Low-cardinality reason an HTTP request was rejected before app handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRejectionReason {
    /// The runtime HTTP in-flight request limit was saturated.
    ConcurrencyLimit,
    /// The request host/authority failed configured validation.
    BlockedHostAuthority,
    /// The request targeted a runtime-owned operational route from a blocked peer.
    BlockedOperationalRoute,
    /// The request targeted a route that is not exposed on this listener visibility.
    BlockedRouteVisibility,
    /// The request exceeded the configured header count limit.
    HeaderCountLimit,
    /// The request exceeded the configured aggregate header byte limit.
    HeaderBytesLimit,
    /// The request ID header was malformed and replaced.
    MalformedRequestId,
    /// The trace ID header was malformed and replaced.
    MalformedTraceId,
    /// Forwarded/proxy headers were stripped because the peer is not trusted.
    UntrustedProxyHeaders,
    /// Forwarded host metadata from a trusted peer was malformed.
    MalformedForwardedHost,
    /// Forwarded scheme/protocol metadata from a trusted peer was malformed.
    MalformedForwardedProto,
    /// Forwarded header families disagreed on the effective external host authority.
    ConflictingForwardedHost,
    /// Forwarded header families disagreed on the effective external scheme/protocol.
    ConflictingForwardedProto,
    /// External scheme validation proved the request was not HTTPS.
    InsecureExternalScheme,
    /// The request content type was rejected before app handling completed.
    InvalidContentType,
    /// Authentication was rejected by a transport/app boundary.
    AuthRejected,
    /// Request exceeded configured route/listener rate limit.
    RateLimited,
    /// Request exceeded configured payload/body cap.
    PayloadTooLarge,
    /// gRPC request exceeded configured message cap.
    MessageTooLarge,
}

impl HttpRejectionReason {
    /// Returns the stable metric label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConcurrencyLimit => "concurrency_limit",
            Self::BlockedHostAuthority => "blocked_host_authority",
            Self::BlockedOperationalRoute => "blocked_operational_route",
            Self::BlockedRouteVisibility => "blocked_route_visibility",
            Self::HeaderCountLimit => "header_count_limit",
            Self::HeaderBytesLimit => "header_bytes_limit",
            Self::MalformedRequestId => "malformed_request_id",
            Self::MalformedTraceId => "malformed_trace_id",
            Self::UntrustedProxyHeaders => "untrusted_proxy_headers",
            Self::MalformedForwardedHost => "malformed_forwarded_host",
            Self::MalformedForwardedProto => "malformed_forwarded_proto",
            Self::ConflictingForwardedHost => "conflicting_forwarded_host",
            Self::ConflictingForwardedProto => "conflicting_forwarded_proto",
            Self::InsecureExternalScheme => "insecure_external_scheme",
            Self::InvalidContentType => "invalid_content_type",
            Self::AuthRejected => "auth_rejected",
            Self::RateLimited => "rate_limited",
            Self::PayloadTooLarge => "payload_too_large",
            Self::MessageTooLarge => "message_too_large",
        }
    }
}

/// Low-cardinality transport label shared by HTTP/connect/gRPC metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportLabel {
    /// Plain HTTP/JSON.
    Http,
    /// Connect over HTTP.
    Connect,
    /// Native gRPC over HTTP/2.
    Grpc,
}

impl TransportLabel {
    /// Returns the stable metric label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Connect => "connect",
            Self::Grpc => "grpc",
        }
    }
}

/// Low-cardinality per-tier rate limiter decision outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRateLimitOutcome {
    /// Request consumed a token and was admitted.
    Allowed,
    /// Request was rejected due to tier/source bucket exhaustion.
    Limited,
    /// Request was rejected because the shared limiter registry reached its bucket cap.
    RegistryFull,
}

impl HttpRateLimitOutcome {
    /// Returns the stable metric label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allowed => "allowed",
            Self::Limited => "limited",
            Self::RegistryFull => "registry_full",
        }
    }
}

/// Low-cardinality bounded runtime queue name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeQueueLabel {
    /// Generic bounded task coordination channel.
    TaskChannel,
    /// Runtime background task tracking capacity.
    BackgroundTaskSet,
    /// WebSocket outbound queue for one managed connection.
    WebSocketOutbound,
}

impl RuntimeQueueLabel {
    /// Returns the stable metric label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaskChannel => "task_channel",
            Self::BackgroundTaskSet => "background_task_set",
            Self::WebSocketOutbound => "websocket_outbound",
        }
    }
}

/// Low-cardinality runtime app lifecycle failure outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAppFailureOutcome {
    /// App startup or cleanup returned a typed failure.
    Failed,
    /// App startup or cleanup exceeded its bounded timeout.
    TimedOut,
}

impl RuntimeAppFailureOutcome {
    /// Returns the stable metric label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }
}

/// Low-cardinality HTTP method label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethodLabel {
    /// HTTP `GET`.
    Get,
    /// HTTP `POST`.
    Post,
    /// HTTP `PUT`.
    Put,
    /// HTTP `PATCH`.
    Patch,
    /// HTTP `DELETE`.
    Delete,
    /// HTTP `OPTIONS`.
    Options,
    /// HTTP `HEAD`.
    Head,
    /// HTTP method outside the standard server-kit set.
    Other,
}

impl HttpMethodLabel {
    /// Maps an HTTP method into a bounded metric label.
    #[cfg(feature = "http")]
    pub fn from_method(value: &Method) -> Self {
        match *value {
            Method::GET => Self::Get,
            Method::POST => Self::Post,
            Method::PUT => Self::Put,
            Method::PATCH => Self::Patch,
            Method::DELETE => Self::Delete,
            Method::OPTIONS => Self::Options,
            Method::HEAD => Self::Head,
            _ => Self::Other,
        }
    }

    /// Returns the stable label value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Options => "OPTIONS",
            Self::Head => "HEAD",
            Self::Other => "OTHER",
        }
    }
}

/// Validated route template label for HTTP metrics.
///
/// Apps should pass stable route templates such as `/readyz` or
/// `/v1/users/:user_id`, never raw paths that contain request-specific values,
/// query strings, or full URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteTemplate(&'static str);

impl RouteTemplate {
    /// Constructs a validated route template label.
    pub fn new(value: &'static str) -> Result<Self, MetricLabelError> {
        if value.is_empty() {
            return Err(MetricLabelError::EmptyRouteTemplate);
        }

        if !value.starts_with('/') {
            return Err(MetricLabelError::RouteTemplateMustStartWithSlash);
        }

        if value.contains('?') {
            return Err(MetricLabelError::RouteTemplateMustNotContainQueryString);
        }

        if value.contains("://") {
            return Err(MetricLabelError::RouteTemplateMustNotContainUrlScheme);
        }

        if value.chars().any(char::is_whitespace) {
            return Err(MetricLabelError::RouteTemplateMustNotContainWhitespace);
        }

        Ok(Self(value))
    }

    /// Returns the route template string.
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// Low-cardinality HTTP status class label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpStatusClass {
    /// `1xx` responses.
    Informational,
    /// `2xx` responses.
    Success,
    /// `3xx` responses.
    Redirection,
    /// `4xx` responses.
    ClientError,
    /// `5xx` responses.
    ServerError,
}

impl HttpStatusClass {
    /// Creates a status class from a numeric HTTP status code.
    pub fn from_status_code(value: u16) -> Self {
        match value / 100 {
            1 => Self::Informational,
            2 => Self::Success,
            3 => Self::Redirection,
            4 => Self::ClientError,
            _ => Self::ServerError,
        }
    }

    /// Returns the stable label value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Informational => "1xx",
            Self::Success => "2xx",
            Self::Redirection => "3xx",
            Self::ClientError => "4xx",
            Self::ServerError => "5xx",
        }
    }
}
