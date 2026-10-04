// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{IdentifierValueError, RequestId, TraceId};

#[test]
fn request_id_parse_round_trips_generated_values() {
    let request_id = RequestId::generate();
    let parsed = RequestId::parse_str(&request_id.to_string());

    assert_eq!(parsed, Ok(request_id));
}

#[test]
fn trace_id_parse_round_trips_generated_values() {
    let trace_id = TraceId::generate();
    let parsed = TraceId::parse_str(&trace_id.to_string());

    assert_eq!(parsed, Ok(trace_id));
}

#[test]
fn request_id_rejects_invalid_uuid_text() {
    let parsed = RequestId::parse_str("not-a-uuid");

    assert_eq!(parsed, Err(IdentifierValueError::InvalidUuid));
}

#[test]
fn trace_id_rejects_invalid_uuid_text() {
    let parsed = TraceId::parse_str("not-a-uuid");

    assert_eq!(parsed, Err(IdentifierValueError::InvalidUuid));
}

#[test]
fn identifiers_require_canonical_lowercase_hyphenated_text() {
    let canonical = "550e8400-e29b-41d4-a716-446655440000";
    assert!(RequestId::parse_str(canonical).is_ok());
    for value in [
        "550E8400-E29B-41D4-A716-446655440000",
        "550e8400e29b41d4a716446655440000",
        "{550e8400-e29b-41d4-a716-446655440000}",
        "urn:uuid:550e8400-e29b-41d4-a716-446655440000",
    ] {
        assert_eq!(
            RequestId::parse_str(value),
            Err(IdentifierValueError::InvalidUuid)
        );
        assert_eq!(
            TraceId::parse_str(value),
            Err(IdentifierValueError::InvalidUuid)
        );
    }
}
