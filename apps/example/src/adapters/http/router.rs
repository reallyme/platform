// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "router_tests.rs"]
mod tests;
