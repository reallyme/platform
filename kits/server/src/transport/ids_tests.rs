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
