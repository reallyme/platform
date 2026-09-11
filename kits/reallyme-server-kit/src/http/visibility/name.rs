// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::HttpRoutePolicyError;
use super::validation::{
    MAX_HTTP_LISTENER_NAME_BYTES, MAX_HTTP_VISIBILITY_CLASS_BYTES, validate_symbolic_name,
};
use std::sync::{Arc, LazyLock};

static UNKNOWN_HTTP_LISTENER_NAME: LazyLock<HttpListenerName> =
    LazyLock::new(|| HttpListenerName(Arc::from("__unknown")));
static DEFAULT_HTTP_LISTENER_NAME: LazyLock<HttpListenerName> =
    LazyLock::new(|| HttpListenerName(Arc::from("http")));

/// Validated custom listener visibility class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpVisibilityClass(String);

impl HttpVisibilityClass {
    /// Creates a custom visibility class.
    pub fn new(value: impl Into<String>) -> Result<Self, HttpRoutePolicyError> {
        let value = value.into();
        validate_symbolic_name(value.as_str(), MAX_HTTP_VISIBILITY_CLASS_BYTES)
            .map_err(|reason| HttpRoutePolicyError::VisibilityClass { reason })?;
        Ok(Self(value))
    }

    /// Returns the validated class name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Validated low-cardinality rate-limit tier name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HttpRateLimitTierName(Arc<str>);

impl HttpRateLimitTierName {
    /// Creates a validated rate-limit tier name.
    pub fn new(value: impl Into<String>) -> Result<Self, HttpRoutePolicyError> {
        let value = value.into();
        validate_symbolic_name(value.as_str(), MAX_HTTP_VISIBILITY_CLASS_BYTES)
            .map_err(|reason| HttpRoutePolicyError::RateLimitTier { reason })?;
        Ok(Self(Arc::from(value)))
    }

    /// Returns the validated tier name.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    pub(crate) fn clone_shared(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }
}

/// Validated low-cardinality auth policy name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpAuthPolicyName(String);

impl HttpAuthPolicyName {
    /// Creates a validated auth policy name.
    pub fn new(value: impl Into<String>) -> Result<Self, HttpRoutePolicyError> {
        let value = value.into();
        validate_symbolic_name(value.as_str(), MAX_HTTP_VISIBILITY_CLASS_BYTES)
            .map_err(|reason| HttpRoutePolicyError::AuthPolicy { reason })?;
        Ok(Self(value))
    }

    /// Returns the validated auth policy name.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Network visibility assigned to one concrete HTTP listener.
///
/// Listener visibility is intentionally runtime-owned and derived from server
/// composition, not request headers. This lets the runtime reject private-only
/// routes on public sockets before app logic runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpListenerVisibility {
    /// Public ingress listener, normally behind Cloudflare, Nginx, or a load balancer.
    Public,
    /// Private listener bound to loopback/VPC/LAN addresses for trusted internal callers.
    Private,
    /// Internal-only listener for administrative or process-local traffic.
    Internal,
    /// Custom deployment-defined visibility class.
    Custom(HttpVisibilityClass),
}

/// Validated listener name used in logs, tests, and future per-listener metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpListenerName(Arc<str>);

impl HttpListenerName {
    /// Creates a validated listener name.
    pub fn new(value: impl Into<String>) -> Result<Self, HttpRoutePolicyError> {
        let value = value.into();

        validate_symbolic_name(value.as_str(), MAX_HTTP_LISTENER_NAME_BYTES)
            .map_err(|reason| HttpRoutePolicyError::ListenerName { reason })?;

        Ok(Self(Arc::from(value)))
    }

    pub(crate) fn unknown_ref() -> &'static Self {
        &UNKNOWN_HTTP_LISTENER_NAME
    }

    /// Returns the validated listener name.
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    pub(crate) fn clone_shared(&self) -> Arc<str> {
        Arc::clone(&self.0)
    }

    /// Returns the canonical default listener name used by HTTP server specs.
    pub(crate) fn http_default() -> Self {
        DEFAULT_HTTP_LISTENER_NAME.clone()
    }
}

/// Runtime-attached identity for the HTTP listener that accepted a request.
///
/// This value is inserted by server-kit middleware after the socket listener is
/// selected. It is intentionally not derived from `Host`, `Forwarded`,
/// `X-Forwarded-*`, or any other client-controlled header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpListenerIdentity {
    name: HttpListenerName,
    visibility: HttpListenerVisibility,
    local_socket_addr: Option<std::net::SocketAddr>,
}

impl HttpListenerIdentity {
    pub(crate) fn new(
        name: HttpListenerName,
        visibility: HttpListenerVisibility,
        local_socket_addr: Option<std::net::SocketAddr>,
    ) -> Self {
        Self {
            name,
            visibility,
            local_socket_addr,
        }
    }

    /// Returns the validated listener name.
    pub const fn name(&self) -> &HttpListenerName {
        &self.name
    }

    /// Returns the server-configured listener visibility.
    pub const fn visibility(&self) -> &HttpListenerVisibility {
        &self.visibility
    }

    /// Returns the raw local socket address for the listener, when available.
    ///
    /// Audit-sensitive apps use this host-supplied value to bind signed reports
    /// to the concrete server socket that accepted the request.
    pub const fn local_socket_addr(&self) -> Option<std::net::SocketAddr> {
        self.local_socket_addr
    }
}
