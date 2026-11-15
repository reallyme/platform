// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum_test::{TestServer, http::StatusCode};
use buffa::Message;
use buffa::view::MessageView;
use bytes::Bytes;
use reallyme_app_kit::ConnectCodeGenerationWorkflow;
use reallyme_example_contract::generated::proto::reallyme::example::v1::{
    HelloRequest, HelloResponse, HelloResponseOwnedView, HelloResponseView,
};

use super::connect_router;
use crate::adapters::connect::{ConnectAdapterConventions, EXAMPLE_HELLO_CONNECT_RPC_PATH};
use crate::app::{ExampleAppConfig, new_context};
use crate::ports::ExamplePorts;

fn local_context() -> crate::app::ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

#[tokio::test]
async fn binary_connect_route_calls_host_neutral_app_core() {
    let server = TestServer::new(connect_router(local_context()).into_axum_router());

    let response = server
        .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
        .bytes(Bytes::from(HelloRequest::default().encode_to_vec()))
        .add_header("content-type", "application/proto")
        .await;

    response.assert_status_ok();
    assert_eq!(
        response.as_bytes().as_ref(),
        b"\x0a\x16hello from example-app"
    );

    let response = HelloResponse::decode(&mut response.as_bytes().as_ref())
        .expect("binary Connect hello response should decode");
    assert_eq!(response.message, "hello from example-app");
}

#[tokio::test]
async fn binary_connect_rejects_a_truncated_protobuf_field() {
    let server = TestServer::new(connect_router(local_context()).into_axum_router());

    let response = server
        .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
        .bytes(Bytes::from_static(b"\x0a\x02a"))
        .add_header("content-type", "application/proto")
        .await;

    response.assert_status_bad_request();
}

#[tokio::test]
async fn native_connect_rejects_json_requests() {
    let server = TestServer::new(connect_router(local_context()).into_axum_router());
    let response = server
        .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
        .bytes(Bytes::from_static(b"{}"))
        .add_header("content-type", "application/json")
        .await;

    response.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[test]
fn public_greeting_round_trips_across_wire_json_and_views() {
    let wire = b"\x0a\x05hello";
    let message = HelloResponse::decode_from_slice(wire).expect("the greeting must decode");
    let borrowed = HelloResponseView::decode_view(wire).expect("the greeting view must decode");
    let owned_view = HelloResponseOwnedView::decode(Bytes::from_static(wire))
        .expect("the owned greeting view must decode");
    let json = serde_json::json!({"message": "hello"});

    assert_eq!(borrowed.message, message.message.as_str());
    assert_eq!(owned_view.to_owned_message(), message);
    assert_eq!(message.encode_to_vec().as_slice(), wire);
    assert_eq!(
        serde_json::to_value(&message).expect("message ProtoJSON"),
        json
    );
    assert_eq!(
        serde_json::to_value(borrowed).expect("borrowed view ProtoJSON"),
        json
    );
    assert_eq!(
        serde_json::to_value(owned_view).expect("owned view ProtoJSON"),
        json
    );
    assert_eq!(
        serde_json::from_value::<HelloResponse>(json).expect("the greeting must decode from JSON"),
        message,
    );
}

#[test]
fn public_contract_rejects_invalid_utf8_and_retains_unknown_wire_fields() {
    let invalid = b"\x0a\x01\xff";
    assert!(HelloResponse::decode_from_slice(invalid).is_err());
    assert!(HelloResponseView::decode_view(invalid).is_err());
    assert!(HelloResponseOwnedView::decode(Bytes::from_static(invalid)).is_err());

    // An unknown field on the public empty request stays round-trippable so
    // older consumers can carry fields introduced by a future schema.
    let unknown = b"\x08\x01";
    let request = HelloRequest::decode_from_slice(unknown).expect("valid unknown protobuf field");
    assert_eq!(request.encode_to_vec().as_slice(), unknown);
}

#[test]
fn generated_protojson_rejects_unknown_fields() {
    assert!(serde_json::from_str::<HelloRequest>(r#"{"unknown":true}"#).is_err());
    assert!(
        serde_json::from_str::<HelloResponse>(r#"{"message":"hello","unknown":true}"#).is_err()
    );
}

#[test]
fn example_connect_adapter_uses_buf_generate_workflow() {
    assert_eq!(
        ConnectAdapterConventions.code_generation_workflow(),
        ConnectCodeGenerationWorkflow::BufGenerateBeforeRustChecks,
    );
}
