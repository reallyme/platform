// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::{Extensions, HeaderMap};

use super::{
    VerifiedTransportSecurity, X_INTERNAL_CALLER, X_SERVICE_TOKEN, attach_internal_request_headers,
    internal_call_correlation_ids_from_extensions,
};
use crate::config::SecretString;
use crate::http::{X_REQUEST_ID, X_TRACE_ID};
use crate::transport::{RequestId, TraceId};

#[test]
fn helper_reuses_existing_correlation_ids_from_extensions() {
    let request_id = RequestId::generate();
    let trace_id = TraceId::generate();
    let mut extensions = Extensions::new();
    extensions.insert(request_id);
    extensions.insert(trace_id);

    let ids = internal_call_correlation_ids_from_extensions(&extensions);

    assert_eq!(ids.request_id(), request_id);
    assert_eq!(ids.trace_id(), trace_id);
}

#[test]
fn helper_attaches_internal_headers_without_logging_secret_values() {
    let extensions = Extensions::new();
    let token = SecretString::new("shared-internal-token".to_owned());
    let mut headers = HeaderMap::new();

    let ids = attach_internal_request_headers(
        &mut headers,
        &extensions,
        "graphql",
        Some(token.expose_secret().as_str()),
    )
    .expect("internal headers should attach");

    assert!(headers.contains_key(X_INTERNAL_CALLER));
    assert!(headers.contains_key(X_SERVICE_TOKEN));
    assert!(headers.contains_key(X_REQUEST_ID));
    assert!(headers.contains_key(X_TRACE_ID));
    assert_ne!(ids.request_id(), RequestId::generate());
    assert_ne!(ids.trace_id(), TraceId::generate());
    assert_eq!(
        headers
            .get(X_INTERNAL_CALLER)
            .and_then(|value| value.to_str().ok()),
        Some("graphql")
    );
}

#[test]
fn verified_transport_security_tracks_mutual_tls_flag() {
    assert!(VerifiedTransportSecurity::mutual_tls_verified().is_mutual_tls_verified());
    assert!(!VerifiedTransportSecurity::not_verified().is_mutual_tls_verified());
}
