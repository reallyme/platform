// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use bytes::Bytes;
use futures_util::stream;
use http_body::Frame;
use http_body_util::StreamBody;
use reqwest::{Body, Response};
use zeroize::ZeroizeOnDrop;

use super::read_response;
use crate::{
    BoundedHttpsResponse, CapturedResponseHeader, HttpsDispatchOutcome, HttpsTransportErrorReason,
};

fn requires_zeroize_on_drop<T: ZeroizeOnDrop>() {}

fn response(status: u16, headers: &[(&str, &str)], body: Vec<u8>) -> http::Response<Body> {
    let mut builder = http::Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    builder.body(Body::from(body)).expect("valid test response")
}

#[tokio::test]
async fn returns_only_bounded_selected_response_data() {
    requires_zeroize_on_drop::<BoundedHttpsResponse>();
    let selection = CapturedResponseHeader::new("x-jku-url", 64).expect("valid selection");
    let response = response(
        201,
        &[
            ("content-type", "application/jwt"),
            ("x-jku-url", "https://keys.example/jwks.json"),
            ("authorization", "must-not-be-retained"),
        ],
        b"signed-value".to_vec(),
    );
    let bounded = read_response(Response::from(response), 64, Some(selection))
        .await
        .expect("bounded response");

    assert_eq!(bounded.status_code(), 201);
    assert_eq!(bounded.content_type(), Some(b"application/jwt".as_slice()));
    assert_eq!(
        bounded.captured_header(),
        Some(b"https://keys.example/jwks.json".as_slice())
    );
    assert_eq!(bounded.body(), b"signed-value");
    assert!(!format!("{bounded:?}").contains("signed-value"));
}

#[tokio::test]
async fn rejects_known_oversized_bodies_after_dispatch() {
    let response = response(200, &[], vec![b'x'; 9]);
    let error = read_response(Response::from(response), 8, None)
        .await
        .expect_err("body must be rejected");
    assert_eq!(
        error.reason(),
        HttpsTransportErrorReason::ResponseLimitExceeded
    );
    assert_eq!(
        error.dispatch_outcome(),
        HttpsDispatchOutcome::RemoteOutcomeUnknown
    );
}

#[tokio::test]
async fn rejects_chunked_bodies_that_cross_the_limit() {
    let chunks = [b"1234".as_slice(), b"56789".as_slice()];
    let body = Body::wrap(StreamBody::new(stream::iter(chunks.into_iter().map(
        |chunk| Ok::<Frame<Bytes>, std::io::Error>(Frame::data(Bytes::copy_from_slice(chunk))),
    ))));
    let response = Response::from(http::Response::new(body));
    let error = read_response(response, 8, None)
        .await
        .expect_err("chunked body must be rejected");
    assert_eq!(
        error.reason(),
        HttpsTransportErrorReason::ResponseLimitExceeded
    );
}

#[tokio::test]
async fn rejects_oversized_selected_headers_without_copying_them() {
    let selection = CapturedResponseHeader::new("x-value", 3).expect("valid selection");
    let response = response(200, &[("x-value", "four")], Vec::new());
    let error = read_response(Response::from(response), 1, Some(selection))
        .await
        .expect_err("header must be rejected");
    assert_eq!(
        error.reason(),
        HttpsTransportErrorReason::ResponseHeaderLimitExceeded
    );
}
