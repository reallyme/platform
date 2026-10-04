// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(feature = "connect-axum")]
use axum::extract::Request;
use axum::extract::State;
#[cfg(feature = "websocket")]
use axum::extract::{ConnectInfo, FromRequestParts};
#[cfg(feature = "websocket")]
use axum::http::request::Parts;
#[cfg(all(feature = "connect-axum", not(feature = "websocket")))]
use axum::response::IntoResponse;
#[cfg(feature = "websocket")]
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
#[cfg(feature = "connect-axum")]
use axum::{
    http::header,
    middleware::{self, Next},
};
#[cfg(feature = "websocket")]
use reallyme_server_kit::config::DEFAULT_WEBSOCKET_CONNECTION_LIMIT;
use reallyme_server_kit::http::{ErrorCode, JsonErrorResponse, PublicHttpError};
#[cfg(feature = "websocket")]
use reallyme_server_kit::http::{
    ExternalRequestOrigin, ForwardedClientIp, NoopWebSocketConnectionHooks,
    WebSocketApplicationMessage, WebSocketConnectionContext, WebSocketConnectionLimiter,
    WebSocketConnectionRuntime, WebSocketMessage, WebSocketMessageHandler, WebSocketUpgrade,
};
#[cfg(feature = "websocket")]
use reallyme_server_kit::http::{
    WebSocketHandlerAction, websocket_same_origin, websocket_upgrade_response,
};
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
/// Builds the example app HTTP router without WebSocket support.
#[cfg(not(feature = "websocket"))]
pub fn router(context: ExampleAppContext) -> Router {
    build_router(context, ())
}

/// Builds the example app HTTP router with an explicit WebSocket shutdown token.
#[cfg(feature = "websocket")]
pub fn router(context: ExampleAppContext, shutdown: ShutdownToken) -> Router {
    build_router(context, shutdown)
}

fn build_router(context: ExampleAppContext, websocket_shutdown: HttpRouterShutdownToken) -> Router {
    let state = ExampleHttpState::new(context.clone(), websocket_shutdown);
    let router = Router::new()
        .route("/hello", get(hello).fallback(method_not_allowed))
        .fallback(not_found);

    #[cfg(feature = "websocket")]
    let router = router.route("/ws", get(websocket_echo).fallback(method_not_allowed));

    #[cfg(feature = "connect-axum")]
    let router = router.merge(
        Router::new()
            .route_service(
                EXAMPLE_HELLO_CONNECT_RPC_PATH,
                connect_router(context.clone()).into_axum_service(),
            )
            .route_layer(middleware::from_fn(connect_http_protocol_policy)),
    );

    router.with_state(state)
}

#[cfg(feature = "connect-axum")]
async fn connect_http_protocol_policy(request: Request, next: Next) -> axum::response::Response {
    // Native gRPC has its own listener policy. The compatibility HTTP route
    // must not accept gRPC framing without that policy's deadline and tiers.
    if request
        .headers()
        .get(header::CONTENT_TYPE)
        .is_some_and(|value| {
            value
                .as_bytes()
                .get(..16)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"application/grpc"))
        })
    {
        return JsonErrorResponse::from_public_error(PublicHttpError::from_code(
            ErrorCode::UnsupportedMediaType,
        ))
        .into_response();
    }

    let mut timeouts = request.headers().get_all("connect-timeout-ms").iter();
    if let Some(timeout) = timeouts.next() {
        let valid = timeouts.next().is_none()
            && timeout.to_str().ok().is_some_and(|value| {
                !value.is_empty()
                    && value.bytes().all(|byte| byte.is_ascii_digit())
                    && value.parse::<u64>().is_ok()
            });
        if !valid {
            return JsonErrorResponse::from_public_error(PublicHttpError::from_code(
                ErrorCode::BadRequest,
            ))
            .into_response();
        }
    }

    if request.method() == axum::http::Method::GET {
        let mut response = JsonErrorResponse::method_not_allowed().into_response();
        response
            .headers_mut()
            .insert(header::ALLOW, axum::http::HeaderValue::from_static("POST"));
        return response;
    }

    next.run(request).await
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
    State(state): State<ExampleHttpState>,
    source: WebSocketSourceIp,
    upgrade: WebSocketUpgrade,
) -> Response {
    if !source.origin_allowed {
        return JsonErrorResponse::from_public_error(PublicHttpError::from_code(
            ErrorCode::Forbidden,
        ))
        .into_response();
    }
    let context = match source.source_ip {
        Some(source_ip) => WebSocketConnectionContext::new(None, None).with_source_ip(source_ip),
        None => WebSocketConnectionContext::new(None, None),
    };
    websocket_upgrade_response(
        upgrade,
        context,
        state.websocket_runtime(),
        ExampleWebSocketEchoHandler,
    )
}

#[cfg(feature = "websocket")]
struct WebSocketSourceIp {
    source_ip: Option<std::net::IpAddr>,
    origin_allowed: bool,
}

#[cfg(feature = "websocket")]
impl<S> FromRequestParts<S> for WebSocketSourceIp
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let ip = parts
            .extensions
            .get::<ForwardedClientIp>()
            .map(|client| client.into_ip_addr())
            .or_else(|| {
                parts
                    .extensions
                    .get::<ConnectInfo<std::net::SocketAddr>>()
                    .map(|peer| peer.0.ip())
            });
        let origin_allowed = websocket_same_origin(
            &parts.headers,
            parts.extensions.get::<ExternalRequestOrigin>(),
        );
        Ok(Self {
            source_ip: ip,
            origin_allowed,
        })
    }
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
        ExampleAppError::DeadlineExceeded => JsonErrorResponse::from_public_error(
            PublicHttpError::from_code(ErrorCode::ServiceUnavailable),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct HelloHttpResponse {
    message: &'static str,
}

#[derive(Clone)]
struct ExampleHttpState {
    context: ExampleAppContext,
    #[cfg(feature = "websocket")]
    websocket_runtime: WebSocketConnectionRuntime,
}

impl ExampleHttpState {
    #[cfg(feature = "websocket")]
    fn new(context: ExampleAppContext, websocket_shutdown: ShutdownToken) -> Self {
        #[cfg(feature = "websocket")]
        let websocket_runtime = WebSocketConnectionRuntime::new(
            reallyme_server_kit::http::WebSocketLimits::safe_defaults(),
            reallyme_server_kit::http::WebSocketHeartbeatConfig::safe_defaults(),
            reallyme_server_kit::http::WebSocketShutdownConfig::safe_defaults(),
            websocket_shutdown,
            std::sync::Arc::new(NoopWebSocketConnectionHooks),
            WebSocketConnectionLimiter::new(DEFAULT_WEBSOCKET_CONNECTION_LIMIT),
        );

        Self {
            context,
            #[cfg(feature = "websocket")]
            websocket_runtime,
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
