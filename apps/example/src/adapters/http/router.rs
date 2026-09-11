// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "websocket")]
use std::sync::Arc;

use axum::extract::State;
#[cfg(feature = "websocket")]
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
#[cfg(feature = "websocket")]
use reallyme_server_kit::config::DEFAULT_WEBSOCKET_CONNECTION_LIMIT;
use reallyme_server_kit::http::{ErrorCode, JsonErrorResponse, PublicHttpError};
#[cfg(feature = "websocket")]
use reallyme_server_kit::http::{
    NoopWebSocketConnectionHooks, WebSocketApplicationMessage, WebSocketConnectionContext,
    WebSocketConnectionLimiter, WebSocketConnectionRuntime, WebSocketMessage,
    WebSocketMessageHandler, WebSocketUpgrade,
};
#[cfg(feature = "websocket")]
use reallyme_server_kit::http::{WebSocketHandlerAction, websocket_upgrade_response};
#[cfg(feature = "websocket")]
use reallyme_server_kit::task::ShutdownController;
#[cfg(feature = "websocket")]
use reallyme_server_kit::task::ShutdownToken;
#[cfg(feature = "websocket")]
type HttpRouterShutdownToken = ShutdownToken;
#[cfg(not(feature = "websocket"))]
type HttpRouterShutdownToken = ();
use serde::Serialize;

#[cfg(feature = "connect-axum")]
use crate::adapters::connect::connect_router;
use crate::app::{ExampleAppContext, ExampleAppError, HelloRequest};
#[cfg(feature = "connect-axum")]
use reallyme_example_contract::EXAMPLE_HELLO_CONNECT_RPC_PATH;

/// Axum adapter that exposes example app behavior through HTTP.
/// Builds the example app HTTP router.
#[cfg_attr(feature = "websocket", allow(dead_code))]
pub fn router(context: ExampleAppContext) -> Router {
    let (router, _state) = build_router(context, None);

    router
}

/// Builds the example app HTTP router with a captured websocket state for process
/// shutdown wiring.
#[cfg(all(feature = "websocket", feature = "native-server"))]
pub(crate) fn router_with_websocket_shutdown(
    context: ExampleAppContext,
) -> (Router, ExampleHttpState) {
    build_router(context, None)
}

fn build_router(
    context: ExampleAppContext,
    _websocket_shutdown: Option<HttpRouterShutdownToken>,
) -> (Router, ExampleHttpState) {
    #[cfg(feature = "websocket")]
    let state = ExampleHttpState::new(context.clone(), _websocket_shutdown);
    #[cfg(not(feature = "websocket"))]
    let state = ExampleHttpState::new(context.clone(), ());
    let router = Router::new()
        .route("/hello", get(hello).fallback(method_not_allowed))
        .fallback(not_found);

    #[cfg(feature = "websocket")]
    let router = router.route("/ws", get(websocket_echo).fallback(method_not_allowed));

    #[cfg(feature = "connect-axum")]
    let router = router.route_service(
        EXAMPLE_HELLO_CONNECT_RPC_PATH,
        connect_router(context.clone()).into_axum_service(),
    );

    (router.with_state(state.clone()), state)
}

async fn hello(
    State(state): State<ExampleHttpState>,
) -> Result<Json<HelloHttpResponse>, JsonErrorResponse> {
    crate::app::hello(state.context(), HelloRequest, None)
        .map(|response| {
            Json(HelloHttpResponse {
                message: response.body(),
            })
        })
        .map_err(map_app_error)
}

#[cfg(feature = "websocket")]
async fn websocket_echo(
    upgrade: WebSocketUpgrade,
    State(state): State<ExampleHttpState>,
) -> Response {
    websocket_upgrade_response(
        upgrade,
        WebSocketConnectionContext::new(None, None),
        state.websocket_runtime(),
        ExampleWebSocketEchoHandler,
    )
}

async fn not_found() -> JsonErrorResponse {
    JsonErrorResponse::not_found()
}

async fn method_not_allowed() -> JsonErrorResponse {
    JsonErrorResponse::method_not_allowed()
}

fn map_app_error(error: ExampleAppError) -> JsonErrorResponse {
    match error {
        ExampleAppError::HelloDisabled => {
            JsonErrorResponse::from_public_error(PublicHttpError::from_code(ErrorCode::Forbidden))
        }
        ExampleAppError::MetricConfigurationInvalid => JsonErrorResponse::from_public_error(
            PublicHttpError::from_code(ErrorCode::InternalServerError),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct HelloHttpResponse {
    message: &'static str,
}

#[derive(Clone)]
pub(crate) struct ExampleHttpState {
    context: ExampleAppContext,
    #[cfg(feature = "websocket")]
    websocket_runtime: WebSocketConnectionRuntime,
    #[cfg(feature = "websocket")]
    _websocket_shutdown: Option<Arc<ShutdownController>>,
}

impl ExampleHttpState {
    #[cfg(feature = "websocket")]
    fn new(context: ExampleAppContext, websocket_shutdown: Option<ShutdownToken>) -> Self {
        let (websocket_shutdown_controller, websocket_shutdown_token) = match websocket_shutdown {
            Some(token) => (None, token),
            None => {
                let controller = Arc::new(ShutdownController::new());
                (Some(Arc::clone(&controller)), controller.token())
            }
        };
        #[cfg(feature = "websocket")]
        let websocket_runtime = WebSocketConnectionRuntime::new(
            reallyme_server_kit::http::WebSocketLimits::safe_defaults(),
            reallyme_server_kit::http::WebSocketHeartbeatConfig::safe_defaults(),
            reallyme_server_kit::http::WebSocketShutdownConfig::safe_defaults(),
            websocket_shutdown_token,
            Arc::new(NoopWebSocketConnectionHooks),
            WebSocketConnectionLimiter::new(DEFAULT_WEBSOCKET_CONNECTION_LIMIT),
        );

        Self {
            context,
            #[cfg(feature = "websocket")]
            websocket_runtime,
            #[cfg(feature = "websocket")]
            _websocket_shutdown: websocket_shutdown_controller,
        }
    }
    #[cfg(not(feature = "websocket"))]
    const fn new(context: ExampleAppContext, _websocket_shutdown: ()) -> Self {
        Self { context }
    }

    const fn context(&self) -> &ExampleAppContext {
        &self.context
    }

    #[cfg(feature = "websocket")]
    fn websocket_runtime(&self) -> WebSocketConnectionRuntime {
        self.websocket_runtime.clone()
    }

    #[cfg(all(feature = "websocket", feature = "native-server"))]
    pub(crate) fn attach_process_shutdown_signal(&self, mut process_shutdown: ShutdownToken) {
        let Some(shutdown_controller) = self._websocket_shutdown.clone() else {
            return;
        };

        tokio::spawn(async move {
            let reason = process_shutdown.cancelled().await;
            let _ = shutdown_controller.begin_shutdown(reason);
        });
    }
}

#[cfg(feature = "websocket")]
struct ExampleWebSocketEchoHandler;

#[cfg(feature = "websocket")]
impl WebSocketMessageHandler for ExampleWebSocketEchoHandler {
    type Error = std::convert::Infallible;
    type HandleFuture<'a> = std::future::Ready<Result<WebSocketHandlerAction, Self::Error>>;

    fn on_message<'a>(
        &'a mut self,
        _context: WebSocketConnectionContext,
        message: WebSocketApplicationMessage,
        outbound: &'a reallyme_server_kit::http::WebSocketConnectionHandle,
    ) -> Self::HandleFuture<'a> {
        match message {
            WebSocketApplicationMessage::Text(text) => {
                if outbound
                    .try_send_message(WebSocketMessage::Text(text))
                    .is_err()
                {
                    // In a product app, this is the place to increment a drop metric
                    // and optionally trace which path hit queue saturation.
                }
            }
            WebSocketApplicationMessage::Binary(binary) => {
                if outbound
                    .try_send_message(WebSocketMessage::Binary(binary))
                    .is_err()
                {
                    // In a product app, this is the place to increment a drop metric
                    // and optionally trace which path hit queue saturation.
                }
            }
        }

        std::future::ready(Ok(WebSocketHandlerAction::Continue))
    }
}

#[cfg(test)]
mod tests {
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

    use crate::app::{ExampleAppConfig, for_tests_only_local_context, new_context};
    use crate::ports::ExamplePorts;

    use super::router;

    #[cfg(feature = "connect-axum")]
    use reallyme_example_contract::EXAMPLE_HELLO_CONNECT_RPC_PATH;

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
        let server = TestServer::new(router(for_tests_only_local_context()));

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
        let server = TestServer::new(router(for_tests_only_local_context()));

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
        let server = TestServer::new(router(for_tests_only_local_context()));

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
            axum::serve(listener, router(for_tests_only_local_context()))
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
}
