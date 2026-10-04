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
#[cfg(feature = "connect-axum")]
use serde::Deserialize;
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
#[cfg(feature = "websocket")]
use reallyme_server_kit::shutdown::ShutdownReason;
#[cfg(feature = "websocket")]
use reallyme_server_kit::task::ShutdownController;

#[cfg(feature = "connect-axum")]
use reallyme_example_contract::EXAMPLE_HELLO_CONNECT_RPC_PATH;

fn local_context() -> crate::app::ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

#[cfg(feature = "connect-axum")]
#[derive(Deserialize)]
struct HostParityCase {
    enabled: bool,
    transport: HostParityTransport,
    expected_status: u16,
    expected_code: Option<String>,
}

#[cfg(feature = "connect-axum")]
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum HostParityTransport {
    HttpHello,
    ConnectHello,
}

#[cfg(feature = "connect-axum")]
#[tokio::test]
async fn native_reference_matches_shared_host_parity_cases() {
    let cases: Vec<HostParityCase> = serde_json::from_str(include_str!(
        "../../../../../conformance/host-parity/cases.json"
    ))
    .expect("valid shared parity cases");
    for case in cases {
        let context = new_context(
            ExampleAppConfig::new(case.enabled),
            ExamplePorts::unconfigured(),
        );
        let (app, _shutdown) = test_router(context);
        let server = TestServer::new(app);
        let response = match case.transport {
            HostParityTransport::HttpHello => server.get("/hello").await,
            HostParityTransport::ConnectHello => {
                server
                    .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
                    .bytes(Bytes::from(HelloRequest::default().encode_to_vec()))
                    .add_header("content-type", "application/proto")
                    .await
            }
        };
        response.assert_status(
            StatusCode::from_u16(case.expected_status).expect("valid fixture status"),
        );
        if case.expected_status == 200 {
            match case.transport {
                HostParityTransport::HttpHello => {
                    response.assert_json(&json!({
                        "message": "hello from example-app"
                    }));
                }
                HostParityTransport::ConnectHello => {
                    let decoded = HelloResponse::decode(&mut response.as_bytes().as_ref())
                        .expect("native Connect response should decode");
                    assert_eq!(decoded.message, "hello from example-app");
                }
            }
        } else {
            let body: serde_json::Value =
                serde_json::from_slice(response.as_bytes().as_ref()).expect("error JSON");
            let observed = match case.transport {
                HostParityTransport::HttpHello => body["error"]["code"].as_str(),
                HostParityTransport::ConnectHello => body["code"].as_str(),
            };
            assert_eq!(observed, case.expected_code.as_deref());
        }
    }
}

#[cfg(feature = "websocket")]
fn test_router(context: crate::app::ExampleAppContext) -> (axum::Router, ShutdownController) {
    let controller = ShutdownController::new();
    (router(context, controller.token()), controller)
}

#[cfg(not(feature = "websocket"))]
fn test_router(context: crate::app::ExampleAppContext) -> (axum::Router, ()) {
    (router(context), ())
}

#[tokio::test]
async fn example_app_exposes_hello_only() {
    let (app, _shutdown) = test_router(local_context());
    let server = TestServer::new(app);

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
    let (app, _shutdown) = test_router(context);
    let server = TestServer::new(app);

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
    let (app, _shutdown) = test_router(local_context());
    let server = TestServer::new(app);

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
    let (app, _shutdown) = test_router(local_context());
    let server = TestServer::new(app);

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

#[cfg(feature = "connect-axum")]
#[tokio::test]
async fn example_http_connect_route_rejects_grpc_and_invalid_timeout_headers() {
    let (app, _shutdown) = test_router(local_context());
    let server = TestServer::new(app);
    let payload = Bytes::from(HelloRequest::default().encode_to_vec());

    let grpc = server
        .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
        .bytes(payload.clone())
        .add_header("content-type", "application/grpc")
        .await;
    grpc.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);

    for timeout in ["abc", "+1000", "18446744073709551616"] {
        let response = server
            .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
            .bytes(payload.clone())
            .add_header("content-type", "application/proto")
            .add_header("connect-timeout-ms", timeout)
            .await;
        response.assert_status(StatusCode::BAD_REQUEST);
    }

    let get = server.get(EXAMPLE_HELLO_CONNECT_RPC_PATH).await;
    get.assert_status(StatusCode::METHOD_NOT_ALLOWED);
    get.assert_header(axum::http::header::ALLOW, "POST");
}

#[tokio::test]
#[cfg(feature = "websocket")]
async fn example_app_exposes_bounded_websocket_echo_route() {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("websocket test listener should bind");
    let address = listener
        .local_addr()
        .expect("websocket test listener should expose a local address");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
    let (app, websocket_shutdown) = test_router(local_context());
    let server_task = tokio::spawn(async move {
        axum::serve(listener, app)
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

    websocket_shutdown.begin_shutdown(ShutdownReason::Sigterm);
    let close = tokio::time::timeout(std::time::Duration::from_secs(2), websocket.next())
        .await
        .expect("websocket should observe shutdown")
        .expect("websocket should receive a close frame")
        .expect("websocket close frame should decode");
    assert!(matches!(close, Message::Close(_)));

    shutdown_sender
        .send(())
        .expect("websocket test server shutdown receiver should be alive");
    server_task
        .await
        .expect("websocket test server task should join cleanly");
}
