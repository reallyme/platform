// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Trusted proxy client-chain parsing with bounded hop traversal.

use std::net::IpAddr;

use axum::http::HeaderMap;

use crate::config::TrustedProxyHeaders;

use super::super::{FORWARDED_HEADER, ForwardedClientIp, X_FORWARDED_FOR_HEADER};

pub(crate) fn forwarded_client_ip_from_headers(
    headers: &HeaderMap,
    trusted_proxies: &TrustedProxyHeaders,
) -> Result<Option<ForwardedClientIp>, ClientChainError> {
    let address = if headers.contains_key(FORWARDED_HEADER) {
        client_ip_from_header_chain(
            headers,
            FORWARDED_HEADER,
            trusted_proxies,
            parse_forwarded_for_ip,
        )
    } else {
        client_ip_from_header_chain(
            headers,
            X_FORWARDED_FOR_HEADER,
            trusted_proxies,
            parse_ip_token,
        )
    };
    address.map(|address| address.map(ForwardedClientIp))
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ClientChainError {
    Malformed,
}

#[derive(Debug, Clone, Copy)]
enum ClientIdentity {
    Address(IpAddr),
    Unspecified,
}

#[cfg(feature = "tonic-grpc")]
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
    .ok()
    .flatten()
}

fn client_ip_from_header_chain(
    headers: &HeaderMap,
    header_name: axum::http::HeaderName,
    trusted_proxies: &TrustedProxyHeaders,
    parse: fn(&str) -> Result<ClientIdentity, ClientChainError>,
) -> Result<Option<IpAddr>, ClientChainError> {
    let mut selected = None;
    let mut count = 0;
    for value in headers.get_all(header_name).iter().rev() {
        let value = value.to_str().map_err(|_| ClientChainError::Malformed)?;
        for token in value.split(',').rev() {
            count += 1;
            if count > 32 {
                return Err(ClientChainError::Malformed);
            }
            let address = match parse(token.trim())? {
                ClientIdentity::Address(address) => address,
                ClientIdentity::Unspecified => return Ok(None),
            };
            selected = Some(address);
            if !trusted_proxies.trusts_peer(Some(address)) {
                return Ok(selected);
            }
        }
    }
    Ok(selected)
}

fn parse_ip_token(value: &str) -> Result<ClientIdentity, ClientChainError> {
    parse_client_identity(value)
}

fn parse_forwarded_for_ip(value: &str) -> Result<ClientIdentity, ClientChainError> {
    let parameter = value.split(';').find_map(|segment| {
        let (key, token) = segment.trim().split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("for")
            .then_some(token.trim())
    });
    let Some(parameter) = parameter else {
        return Ok(ClientIdentity::Unspecified);
    };
    let parameter = parameter.trim_matches('"');
    parse_client_identity(parameter)
}

fn parse_client_identity(value: &str) -> Result<ClientIdentity, ClientChainError> {
    if value.eq_ignore_ascii_case("unknown") || value.starts_with('_') {
        return Ok(ClientIdentity::Unspecified);
    }
    if let Some(bracketed) = value.strip_prefix('[') {
        let (address, suffix) = bracketed
            .split_once(']')
            .ok_or(ClientChainError::Malformed)?;
        if let Some(port) = suffix.strip_prefix(':') {
            port.parse::<u16>()
                .map_err(|_| ClientChainError::Malformed)?;
        } else if !suffix.is_empty() {
            return Err(ClientChainError::Malformed);
        }
        address
            .parse::<IpAddr>()
            .map(ClientIdentity::Address)
            .map_err(|_| ClientChainError::Malformed)
    } else {
        let address = value.parse::<IpAddr>().or_else(|_| {
            let (address, port) = value.rsplit_once(':').ok_or(ClientChainError::Malformed)?;
            port.parse::<u16>()
                .map_err(|_| ClientChainError::Malformed)?;
            address
                .parse::<IpAddr>()
                .map_err(|_| ClientChainError::Malformed)
        })?;
        Ok(ClientIdentity::Address(address))
    }
}
