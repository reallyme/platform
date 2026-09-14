// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::StatusCode;
use axum_test::TestServer;
#[cfg(feature = "connect-axum")]
use buffa::Message as BuffaMessage;
#[cfg(feature = "connect-axum")]
use bytes::Bytes;
#[cfg(feature = "websocket")]
use futures_util::{SinkExt, StreamExt};
#[cfg(feature = "connect-axum")]
use reallyme_example_contract::generated::proto::reallyme::example::v1::{
    HelloRequest, HelloResponse,
};
use serde_json::json;
#[cfg(feature = "websocket")]
use tokio::sync::oneshot;
#[cfg(feature = "websocket")]
use tokio_tungstenite::connect_async;
#[cfg(feature = "websocket")]
use tokio_tungstenite::tungstenite::Message;

use crate::app::{ExampleAppConfig, new_context};
use crate::ports::ExamplePorts;

use super::router;

#[cfg(feature = "connect-axum")]
use reallyme_example_contract::EXAMPLE_HELLO_CONNECT_RPC_PATH;

fn local_context() -> crate::app::ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

#[cfg(feature = "websocket")]
async fn bind_websocket_test_listener() -> Option<tokio::net::TcpListener> {
    match tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await {
        Ok(listener) => Some(listener),
        // WebSocket tests require real socket access. CI must provide
        // sockets so the full route is exercised; this branch keeps
        // restricted local sandboxes from failing before app code runs.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => None,
        Err(error) => panic!("websocket test listener preflight should bind: {error}"),
    }
}

#[tokio::test]
async fn example_app_exposes_hello_only() {
    let server = TestServer::new(router(local_context()));

    let hello = server.get("/hello").await;
    let readyz = server.get("/readyz").await;
    #[cfg(feature = "websocket")]
    let websocket_probe = server.get("/ws").await;

    hello.assert_json(&json!({
        "message": "hello from example-app"
    }));
    readyz.assert_status_not_found();
    readyz.assert_json(&json!({
        "error": {
            "code": "not_found",
            "message": "Not found"
        }
    }));
    #[cfg(feature = "websocket")]
    websocket_probe.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn example_http_adapter_maps_app_errors_to_stable_envelope() {
    let context = new_context(ExampleAppConfig::new(false), ExamplePorts::unconfigured());
    let server = TestServer::new(router(context));

    let response = server.get("/hello").await;

    response.assert_status(StatusCode::FORBIDDEN);
    response.assert_json(&json!({
        "error": {
            "code": "forbidden",
            "message": "Forbidden"
        }
    }));
}

#[tokio::test]
async fn example_http_adapter_maps_method_mismatch_to_stable_envelope() {
    let server = TestServer::new(router(local_context()));

    let response = server.post("/hello").await;

    response.assert_status(StatusCode::METHOD_NOT_ALLOWED);
    response.assert_json(&json!({
        "error": {
            "code": "method_not_allowed",
            "message": "Method not allowed"
        }
    }));
}

#[tokio::test]
#[cfg(feature = "connect-axum")]
async fn example_app_exposes_generated_connect_rpc_route() {
    let server = TestServer::new(router(local_context()));

    let response = server
        .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
        .bytes(Bytes::from(HelloRequest::default().encode_to_vec()))
        .add_header("content-type", "application/proto")
        .await;

    response.assert_status_ok();
    let response = HelloResponse::decode(&mut response.as_bytes().as_ref())
        .expect("binary Connect example hello response should decode");
    assert_eq!(response.message, "hello from example-app");
}

#[tokio::test]
#[cfg(feature = "websocket")]
async fn example_app_exposes_bounded_websocket_echo_route() {
    let Some(listener) = bind_websocket_test_listener().await else {
        return;
    };
    let address = listener
        .local_addr()
        .expect("websocket test listener should expose a local address");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let server_task = tokio::spawn(async move {
        axum::serve(listener, router(local_context()))
            .with_graceful_shutdown(async move {
                let _shutdown_requested = shutdown_receiver.await;
            })
            .await
            .expect("websocket test server should run");
    });

    let (mut websocket, _response) = connect_async(format!("ws://{address}/ws"))
        .await
        .expect("websocket test client should connect");

    websocket
        .send(Message::Text("hello from websocket".into()))
        .await
        .expect("websocket test client should send text");
    let message = websocket
        .next()
        .await
        .expect("websocket test client should receive a message")
        .expect("websocket frame should be valid");

    assert_eq!(
        message
            .into_text()
            .expect("websocket echo should be a text frame")
            .as_str(),
        "hello from websocket"
    );

    shutdown_sender
        .send(())
        .expect("websocket test server shutdown receiver should be alive");
    server_task
        .await
        .expect("websocket test server task should join cleanly");
}
