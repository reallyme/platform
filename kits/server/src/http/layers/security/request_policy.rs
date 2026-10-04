// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Request-scoped host, peer, operational-route, and rejection policy helpers.

use std::net::IpAddr;

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{Request, Version, header};

use super::super::listener::listener_name_for_request;
use super::ForwardedHost;
use crate::config::{HostAuthority, HostAuthorityPolicy, OperationalRouteAccess};

pub(super) fn request_host_header_is_valid(request: &Request<Body>) -> bool {
    let mut hosts = request.headers().get_all(header::HOST).iter();
    let host = hosts.next();
    if hosts.next().is_some() || (request.version() == Version::HTTP_11 && host.is_none()) {
        return false;
    }
    let Some(host) = host else {
        return true;
    };
    let Some(host) = host
        .to_str()
        .ok()
        .and_then(|value| HostAuthority::retain_authority(value).ok())
    else {
        return false;
    };
    request.uri().authority().is_none_or(|authority| {
        HostAuthority::retain_authority(authority.as_str()).is_ok_and(|uri_host| {
            uri_host.host().eq_ignore_ascii_case(host.host()) && uri_host.port() == host.port()
        })
    })
}
use crate::http::routes::{HEALTHZ_PATH, METRICS_PATH, READYZ_PATH, VERSION_PATH};
use crate::observability::{
    HttpMethodLabel, HttpRejectionReason, MetricRouteTemplateLabel, UNKNOWN_ROUTE_TEMPLATE,
    record_http_request_rejected_for_route_template,
};

pub(super) fn route_template_for_request(request: &Request<Body>) -> &str {
    request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or(UNKNOWN_ROUTE_TEMPLATE)
}

pub(super) fn record_security_rejection(
    request: &Request<Body>,
    method: HttpMethodLabel,
    reason: HttpRejectionReason,
) {
    let route_template =
        MetricRouteTemplateLabel::from_matched_path(request.extensions().get::<MatchedPath>());
    record_http_request_rejected_for_route_template(
        listener_name_for_request(request),
        method,
        &route_template,
        reason,
    );
}

pub(super) fn peer_ip_from_request(request: &Request<Body>) -> Option<IpAddr> {
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
}

pub(super) fn host_authority_is_allowed(
    authority: Option<&ForwardedHost>,
    policy: &HostAuthorityPolicy,
) -> bool {
    match authority {
        Some(authority) => policy.allows_normalized(authority.authority()),
        None => matches!(policy, HostAuthorityPolicy::Any),
    }
}

pub(super) fn operational_route_is_blocked(
    request: &Request<Body>,
    access: OperationalRouteAccess,
    contains_proxy_headers: bool,
) -> bool {
    if access == OperationalRouteAccess::Public || !is_operational_route(request.uri().path()) {
        return false;
    }

    contains_proxy_headers
        || request
            .extensions()
            .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
            .is_none_or(|connect_info| !ip_is_loopback(connect_info.0.ip()))
}

pub(super) fn is_operational_route(path: &str) -> bool {
    matches!(
        path,
        HEALTHZ_PATH | READYZ_PATH | VERSION_PATH | METRICS_PATH
    )
}

fn ip_is_loopback(ip: IpAddr) -> bool {
    ip.is_loopback()
}
