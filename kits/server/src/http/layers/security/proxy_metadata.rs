// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Trusted reverse-proxy metadata normalization.

use std::net::IpAddr;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, header};
use tracing::debug;

use crate::config::{HostAuthority, NetworkPort, TrustedProxyHeaderFamily, TrustedProxyHeaders};

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

pub(super) fn strip_unselected_proxy_headers(
    request: &mut Request<Body>,
    family: TrustedProxyHeaderFamily,
) {
    let headers = request.headers_mut();
    match family {
        TrustedProxyHeaderFamily::Forwarded => {
            headers.remove(X_FORWARDED_FOR_HEADER);
            headers.remove(X_FORWARDED_HOST_HEADER);
            headers.remove(X_FORWARDED_PORT_HEADER);
            headers.remove(X_FORWARDED_PROTO_HEADER);
            headers.remove(X_REAL_IP_HEADER);
        }
        TrustedProxyHeaderFamily::XForwarded => {
            headers.remove(FORWARDED_HEADER);
            headers.remove(X_REAL_IP_HEADER);
        }
    }
}

pub(super) fn has_mixed_proxy_header_families(headers: &HeaderMap) -> bool {
    headers.contains_key(FORWARDED_HEADER)
        && (headers.contains_key(X_FORWARDED_FOR_HEADER)
            || headers.contains_key(X_FORWARDED_HOST_HEADER)
            || headers.contains_key(X_FORWARDED_PORT_HEADER)
            || headers.contains_key(X_FORWARDED_PROTO_HEADER))
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

    // Absolute-form request targets are controlled by the caller and cannot
    // attest to transport TLS. A direct request has no proxy scheme proof.
    Ok(None)
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
        if headers.get_all(X_FORWARDED_HOST_HEADER).iter().count() != 1 {
            return Err(ProxyMetadataError::InvalidForwardedHost);
        }
        let authority = last_comma_separated_token(value)
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
        if headers.get_all(X_FORWARDED_PROTO_HEADER).iter().count() != 1 {
            return Err(ProxyMetadataError::InvalidForwardedProto);
        }
        let proto = last_comma_separated_token(value)
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
    if headers.get_all(X_FORWARDED_PORT_HEADER).iter().count() != 1 {
        return Err(ProxyMetadataError::InvalidForwardedHost);
    }
    let token =
        last_comma_separated_token(value).ok_or(ProxyMetadataError::InvalidForwardedHost)?;
    let port = token
        .parse::<u16>()
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)?;

    NetworkPort::new(port)
        .map(Some)
        .map_err(|_| ProxyMetadataError::InvalidForwardedHost)
}

pub(super) fn forwarded_client_ip_from_headers(
    headers: &HeaderMap,
    trusted_proxies: &TrustedProxyHeaders,
) -> Option<ForwardedClientIp> {
    let address = if headers.contains_key(FORWARDED_HEADER) {
        client_ip_from_header_chain(
            headers,
            FORWARDED_HEADER,
            trusted_proxies,
            parse_forwarded_for_ip,
        )
    } else {
        client_ip_from_x_forwarded_for(headers, trusted_proxies)
    };
    address.map(ForwardedClientIp)
}

pub(crate) fn client_ip_from_x_forwarded_for(
    headers: &HeaderMap,
    trusted_proxies: &TrustedProxyHeaders,
) -> Option<IpAddr> {
    client_ip_from_header_chain(
        headers,
        X_FORWARDED_FOR_HEADER,
        trusted_proxies,
        parse_ip_token,
    )
}

fn client_ip_from_header_chain(
    headers: &HeaderMap,
    header_name: axum::http::HeaderName,
    trusted_proxies: &TrustedProxyHeaders,
    parse: fn(&str) -> Option<IpAddr>,
) -> Option<IpAddr> {
    let mut values = headers.get_all(header_name).iter();
    let value = values.next()?;
    if values.next().is_some() {
        return None;
    }

    let mut selected = None;
    for (index, token) in value.to_str().ok()?.split(',').rev().enumerate() {
        if index >= 32 {
            return None;
        }
        let address = parse(token.trim())?;
        selected = Some(address);
        if !trusted_proxies.trusts_peer(Some(address)) {
            break;
        }
    }
    selected
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

fn last_comma_separated_token(value: &HeaderValue) -> Option<&str> {
    let value = value.to_str().ok()?;
    let last = value.split(',').next_back()?.trim();
    if last.is_empty() {
        return None;
    }

    Some(last)
}

fn parse_ip_token(value: &str) -> Option<IpAddr> {
    value.parse::<IpAddr>().ok()
}

fn parse_forwarded_for_ip(value: &str) -> Option<IpAddr> {
    let parameter = value.split(';').find_map(|segment| {
        let (key, token) = segment.trim().split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("for")
            .then_some(token.trim())
    })?;
    let parameter = parameter.trim_matches('"');
    if let Some(bracketed) = parameter.strip_prefix('[') {
        let (address, suffix) = bracketed.split_once(']')?;
        if !suffix.is_empty() && !suffix.starts_with(':') {
            return None;
        }
        address.parse::<IpAddr>().ok()
    } else {
        parameter.parse::<IpAddr>().ok()
    }
}

fn forwarded_header_parameter<'a>(value: &'a HeaderValue, key: &str) -> Option<&'a str> {
    let value = value.to_str().ok()?;
    let nearest_forwarded = value.split(',').next_back()?.trim();
    nearest_forwarded.split(';').find_map(|segment| {
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
