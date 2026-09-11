// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::error::HephaestusAgentErrorReason;

use super::{MAX_DOCKER_RESPONSE_BYTES, append_response_chunk, parse_response, validate_path};

#[test]
fn parses_docker_http_response() {
    let response = parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
        .unwrap_or_else(|error| panic!("valid docker response: {error:?}"));

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body(), b"{}");
}

#[test]
fn parses_chunked_docker_http_response() {
    let response = parse_response(
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n",
    )
    .unwrap_or_else(|error| panic!("valid chunked docker response: {error:?}"));

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body(), b"{}");
}

#[test]
fn rejects_malformed_chunked_docker_http_response() {
    let response =
        parse_response(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\n0\r\n\r\n");

    assert!(response.is_err());
}

#[test]
fn rejects_unsafe_api_path() {
    assert!(validate_path("/containers/../../stop").is_err());
}

#[test]
fn rejects_invalid_percent_encoding_in_api_path() {
    assert!(validate_path("/images/reallyme%ZZnats/json").is_err());
    assert!(validate_path("/images/reallyme%/json").is_err());
}

#[test]
fn bounded_response_accumulation_rejects_payload_above_limit() {
    let mut response = vec![0u8; MAX_DOCKER_RESPONSE_BYTES];
    let result = append_response_chunk(&mut response, &[0u8]);

    assert_eq!(
        result.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::CommandOutputInvalid)
    );
}

#[test]
fn bounded_response_accumulation_allows_exact_limit() {
    let mut response = vec![0u8; MAX_DOCKER_RESPONSE_BYTES];

    assert!(append_response_chunk(&mut response, &[]).is_ok());
}
