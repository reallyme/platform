// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{HeaderMap, Uri, header};
use url::Url;

use crate::http::ExternalRequestOrigin;

const MAX_WEBSOCKET_ORIGIN_BYTES: usize = 2_048;

/// Checks browser WebSocket origins against the externally visible endpoint.
///
/// Browsers send `Origin` on WebSocket handshakes. Requests without it can be
/// non-browser clients, which must still authenticate through the app's own
/// transport policy. A present but malformed or cross-origin value is denied.
pub fn websocket_same_origin(headers: &HeaderMap, uri: &Uri) -> bool {
    websocket_same_origin_with_external_origin(headers, uri, None)
}

/// Checks the origin against a trusted externally visible endpoint when present.
pub fn websocket_same_origin_with_external_origin(
    headers: &HeaderMap,
    uri: &Uri,
    external_origin: Option<&ExternalRequestOrigin>,
) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    let Some(origin) = origins.next() else {
        return true;
    };
    if origins.next().is_some() {
        return false;
    }
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    if origin.is_empty() || origin.len() > MAX_WEBSOCKET_ORIGIN_BYTES || origin.contains('\\') {
        return false;
    }
    let Ok(parsed_origin) = Url::parse(origin) else {
        return false;
    };
    if !matches!(parsed_origin.scheme(), "http" | "https")
        || parsed_origin.host().is_none()
        || !parsed_origin.username().is_empty()
        || parsed_origin.password().is_some()
        || parsed_origin.query().is_some()
        || parsed_origin.fragment().is_some()
        || parsed_origin.path() != "/"
        || parsed_origin.origin().ascii_serialization() != origin
    {
        return false;
    }

    let expected = match external_origin {
        Some(trusted) => format!(
            "{}://{}",
            trusted.proto().as_str(),
            trusted.host().authority().as_str(),
        ),
        None => {
            let mut hosts = headers.get_all(header::HOST).iter();
            let first_host = hosts.next();
            if hosts.next().is_some() {
                return false;
            }
            let host = match first_host {
                Some(host) => match host.to_str() {
                    Ok(host) => host,
                    Err(_) => return false,
                },
                None => match uri.authority() {
                    Some(authority) => authority.as_str(),
                    None => return false,
                },
            };
            if uri
                .authority()
                .is_some_and(|authority| authority.as_str() != host)
            {
                return false;
            }
            let scheme = match uri.scheme_str() {
                Some("https") => "https",
                Some("http") | None => "http",
                Some(_) => return false,
            };
            format!("{scheme}://{host}")
        }
    };
    Url::parse(&expected).is_ok_and(|expected| expected.origin() == parsed_origin.origin())
}

#[cfg(test)]
#[path = "origin_tests.rs"]
mod tests;
