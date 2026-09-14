// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Trusted reverse-proxy metadata normalization.

use std::net::IpAddr;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, header};
use tracing::debug;

use crate::config::{HostAuthority, NetworkPort};

use super::{
    FORWARDED_HEADER, ForwardedClientIp, ForwardedHost, ForwardedMetadataDebugReason,
    ForwardedProto, ProxyMetadataError, X_FORWARDED_FOR_HEADER, X_FORWARDED_HOST_HEADER,
    X_FORWARDED_PORT_HEADER, X_FORWARDED_PROTO_HEADER, X_REAL_IP_HEADER, is_operational_route,
};

pub(super) fn strip_untrusted_proxy_headers(request: &mut Request<Body>) {
    let headers = request.headers_mut();
    headers.remove(FORWARDED_HEADER);
    headers.remove(X_FORWARDED_FOR_HEADER);
    headers.remove(X_FORWARDED_HOST_HEADER);
    headers.remove(X_FORWARDED_PORT_HEADER);
    headers.remove(X_FORWARDED_PROTO_HEADER);
    headers.remove(X_REAL_IP_HEADER);
}

pub(super) fn request_contains_proxy_headers(request: &Request<Body>) -> bool {
    let headers = request.headers();

    headers.contains_key(FORWARDED_HEADER)
        || headers.contains_key(X_FORWARDED_FOR_HEADER)
        || headers.contains_key(X_FORWARDED_HOST_HEADER)
        || headers.contains_key(X_FORWARDED_PORT_HEADER)
        || headers.contains_key(X_FORWARDED_PROTO_HEADER)
        || headers.contains_key(X_REAL_IP_HEADER)
}

pub(super) fn normalized_external_host(
    request: &Request<Body>,
    trusted_peer: bool,
    trust_forwarded_host: bool,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    if trusted_peer && trust_forwarded_host {
        let forwarded = forwarded_host_from_forwarded_header(request.headers());
        let x_forwarded = forwarded_host_from_x_forwarded_headers(request.headers());

        if let Some(authority) = resolve_forwarded_host_precedence(
            forwarded,
            x_forwarded,
            strict_forwarded_header_consistency,
            listener_name,
            route_template,
        )? {
            return Ok(Some(authority));
        }
    }

    Ok(direct_host_authority(request).map(ForwardedHost::new))
}

pub(super) fn normalized_external_proto(
    request: &Request<Body>,
    trusted_peer: bool,
    trust_forwarded_proto: bool,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    if trusted_peer && trust_forwarded_proto {
        let forwarded = forwarded_proto_from_forwarded_header(request.headers());
        let x_forwarded = forwarded_proto_from_x_forwarded_headers(request.headers());

        if let Some(proto) = resolve_forwarded_proto_precedence(
            forwarded,
            x_forwarded,
            strict_forwarded_header_consistency,
            listener_name,
            route_template,
        )? {
            return Ok(Some(proto));
        }
    }

    Ok(request.uri().scheme_str().and_then(ForwardedProto::parse))
}

impl ForwardedProto {
    fn parse(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "http" => Some(Self::Http),
            "https" => Some(Self::Https),
            _ => None,
        }
    }
}

fn direct_host_authority(request: &Request<Body>) -> Option<HostAuthority> {
    request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| HostAuthority::retain_authority(value).ok())
        .or_else(|| {
            request
                .uri()
                .authority()
                .and_then(|value| HostAuthority::retain_authority(value.as_str()).ok())
        })
}

fn forwarded_host_from_forwarded_header(
    headers: &HeaderMap,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    if let Some(value) = headers.get(FORWARDED_HEADER)
        && let Some(host) = forwarded_header_parameter(value, "host")
    {
        let authority = HostAuthority::retain_authority(host)
            .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;
        return Ok(Some(ForwardedHost::new(authority)));
    }

    Ok(None)
}

fn forwarded_host_from_x_forwarded_headers(
    headers: &HeaderMap,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    if let Some(value) = headers.get(X_FORWARDED_HOST_HEADER) {
        let authority = first_comma_separated_token(value)
            .ok_or(ProxyMetadataError::InvalidForwardedHost)
            .and_then(|host| forwarded_host_authority_from_x_forwarded_headers(headers, host))?;
        return Ok(Some(ForwardedHost::new(authority)));
    }

    Ok(None)
}

fn resolve_forwarded_host_precedence(
    forwarded: Result<Option<ForwardedHost>, ProxyMetadataError>,
    x_forwarded: Result<Option<ForwardedHost>, ProxyMetadataError>,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedHost>, ProxyMetadataError> {
    match (forwarded, x_forwarded) {
        (Ok(Some(forwarded_host)), Ok(Some(x_forwarded_host))) => {
            if forwarded_host != x_forwarded_host {
                if strict_forwarded_header_consistency {
                    return Err(ProxyMetadataError::ConflictingForwardedHost);
                }

                debug_forwarded_metadata_reason(
                    ForwardedMetadataDebugReason::ConflictingForwardedHost,
                    listener_name,
                    route_template,
                );
            }

            Ok(Some(forwarded_host))
        }
        (Ok(Some(forwarded_host)), Err(_)) => {
            debug_forwarded_metadata_reason(
                ForwardedMetadataDebugReason::InvalidFallbackForwardedHost,
                listener_name,
                route_template,
            );
            Ok(Some(forwarded_host))
        }
        (Ok(Some(forwarded_host)), Ok(None)) => Ok(Some(forwarded_host)),
        (Err(error), _) => Err(error),
        (Ok(None), Ok(Some(x_forwarded_host))) => Ok(Some(x_forwarded_host)),
        (Ok(None), Err(error)) => Err(error),
        (Ok(None), Ok(None)) => Ok(None),
    }
}

fn forwarded_host_authority_from_x_forwarded_headers(
    headers: &HeaderMap,
    value: &str,
) -> Result<HostAuthority, ProxyMetadataError> {
    let parts = HostAuthority::authority_parts(value)
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;
    if parts.port().is_some() {
        return HostAuthority::retain_authority(value)
            .map_err(|_| ProxyMetadataError::InvalidForwardedHost);
    }

    let Some(port) = forwarded_port_from_headers(headers)? else {
        return HostAuthority::retain_authority(value)
            .map_err(|_| ProxyMetadataError::InvalidForwardedHost);
    };

    HostAuthority::new(format_authority_with_port(parts.host(), port))
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)
}

fn forwarded_proto_from_forwarded_header(
    headers: &HeaderMap,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    if let Some(value) = headers.get(FORWARDED_HEADER)
        && let Some(proto) = forwarded_header_parameter(value, "proto")
    {
        return parse_forwarded_proto_token(proto).map(Some);
    }

    Ok(None)
}

fn forwarded_proto_from_x_forwarded_headers(
    headers: &HeaderMap,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    if let Some(value) = headers.get(X_FORWARDED_PROTO_HEADER) {
        let proto = first_comma_separated_token(value)
            .ok_or(ProxyMetadataError::InvalidForwardedProto)
            .and_then(parse_forwarded_proto_token)?;
        return Ok(Some(proto));
    }

    Ok(None)
}

fn resolve_forwarded_proto_precedence(
    forwarded: Result<Option<ForwardedProto>, ProxyMetadataError>,
    x_forwarded: Result<Option<ForwardedProto>, ProxyMetadataError>,
    strict_forwarded_header_consistency: bool,
    listener_name: &str,
    route_template: &str,
) -> Result<Option<ForwardedProto>, ProxyMetadataError> {
    match (forwarded, x_forwarded) {
        (Ok(Some(forwarded_proto)), Ok(Some(x_forwarded_proto))) => {
            if forwarded_proto != x_forwarded_proto {
                if strict_forwarded_header_consistency {
                    return Err(ProxyMetadataError::ConflictingForwardedProto);
                }

                debug_forwarded_metadata_reason(
                    ForwardedMetadataDebugReason::ConflictingForwardedProto,
                    listener_name,
                    route_template,
                );
            }

            Ok(Some(forwarded_proto))
        }
        (Ok(Some(forwarded_proto)), Err(_)) => {
            debug_forwarded_metadata_reason(
                ForwardedMetadataDebugReason::InvalidFallbackForwardedProto,
                listener_name,
                route_template,
            );
            Ok(Some(forwarded_proto))
        }
        (Ok(Some(forwarded_proto)), Ok(None)) => Ok(Some(forwarded_proto)),
        (Err(error), _) => Err(error),
        (Ok(None), Ok(Some(x_forwarded_proto))) => Ok(Some(x_forwarded_proto)),
        (Ok(None), Err(error)) => Err(error),
        (Ok(None), Ok(None)) => Ok(None),
    }
}

fn parse_forwarded_proto_token(value: &str) -> Result<ForwardedProto, ProxyMetadataError> {
    ForwardedProto::parse(value).ok_or(ProxyMetadataError::InvalidForwardedProto)
}

fn forwarded_port_from_headers(
    headers: &HeaderMap,
) -> Result<Option<NetworkPort>, ProxyMetadataError> {
    let Some(value) = headers.get(X_FORWARDED_PORT_HEADER) else {
        return Ok(None);
    };
    let token =
        first_comma_separated_token(value).ok_or(ProxyMetadataError::InvalidForwardedHost)?;
    let port = token
        .parse::<u16>()
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;

    NetworkPort::new(port)
        .map(Some)
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)
}

pub(super) fn forwarded_client_ip_from_headers(headers: &HeaderMap) -> Option<ForwardedClientIp> {
    headers
        .get(FORWARDED_HEADER)
        .and_then(forwarded_header_for_ip)
        .or_else(|| {
            headers
                .get(X_FORWARDED_FOR_HEADER)
                .and_then(first_forwarded_for_ip)
        })
        .or_else(|| headers.get(X_REAL_IP_HEADER).and_then(header_value_ip))
        .map(ForwardedClientIp)
}

pub(super) fn direct_request_without_https_proof_allowed(
    request: &Request<Body>,
    peer_ip: Option<IpAddr>,
) -> bool {
    is_operational_route(request.uri().path()) && peer_ip.is_some_and(ip_is_loopback_or_private)
}

fn ip_is_loopback_or_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(address) => {
            address.is_loopback() || address.is_private() || address.is_link_local()
        }
        IpAddr::V6(address) => {
            address.is_loopback() || address.is_unique_local() || address.is_unicast_link_local()
        }
    }
}

fn debug_forwarded_metadata_reason(
    reason: ForwardedMetadataDebugReason,
    listener_name: &str,
    route_template: &str,
) {
    debug!(
        listener_name,
        route_template,
        reason = reason.as_str(),
        "trusted_proxy_metadata_normalized_by_precedence"
    );
}

fn first_comma_separated_token(value: &HeaderValue) -> Option<&str> {
    let value = value.to_str().ok()?;
    let first = value.split(',').next()?.trim();
    if first.is_empty() {
        return None;
    }

    Some(first)
}

fn first_forwarded_for_ip(value: &HeaderValue) -> Option<IpAddr> {
    let first = first_comma_separated_token(value)?;

    first.parse::<IpAddr>().ok()
}

fn header_value_ip(value: &HeaderValue) -> Option<IpAddr> {
    value.to_str().ok()?.trim().parse::<IpAddr>().ok()
}

fn forwarded_header_for_ip(value: &HeaderValue) -> Option<IpAddr> {
    let first_for = forwarded_header_parameter(value, "for")?;
    let normalized = first_for
        .trim_matches('"')
        .trim_start_matches('[')
        .trim_end_matches(']');

    normalized.parse::<IpAddr>().ok()
}

fn forwarded_header_parameter<'a>(value: &'a HeaderValue, key: &str) -> Option<&'a str> {
    let value = value.to_str().ok()?;
    let first_forwarded = value.split(',').next()?.trim();
    first_forwarded.split(';').find_map(|segment| {
        let (segment_key, segment_value) = segment.trim().split_once('=')?;
        if !segment_key.trim().eq_ignore_ascii_case(key) {
            return None;
        }

        let normalized = segment_value.trim().trim_matches('"');
        if normalized.is_empty() {
            return None;
        }

        Some(normalized)
    })
}

fn format_authority_with_port(host: &str, port: NetworkPort) -> String {
    if host.contains(':') {
        format!("[{host}]:{}", port.as_u16())
    } else {
        format!("{host}:{}", port.as_u16())
    }
}
