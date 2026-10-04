// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use super::{
    ApplicationMessageInput, ApplicationMessageOutcome, ConnectionId, NoopWebSocketConnectionHooks,
    WebSocketApplicationMessage, WebSocketConnectionContext, WebSocketConnectionHandle,
    WebSocketConnectionLimiter, WebSocketConnectionRuntime, WebSocketHandlerAction,
    WebSocketMessageHandler, configure_websocket_upgrade, handle_application_message,
    websocket_protocol_close_reason, websocket_upgrade_response,
};
use crate::config::{ConcurrencyLimitConfigField, RuntimeConcurrencyLimit};
use crate::http::WebSocketUpgrade;
use crate::http::websocket::{
    HeartbeatInterval, IdleTimeout, OutboundQueueCapacity, WebSocketConnectionLimitError,
    WebSocketConnectionLimitErrorReason, WebSocketHeartbeatConfig, WebSocketLimits,
    WebSocketSendError, WebSocketSendErrorReason, WebSocketShutdownConfig,
};
use crate::task::ShutdownController;

fn make_outbound_channel(
    connection_id: ConnectionId,
    capacity: usize,
) -> (
    WebSocketConnectionHandle,
    mpsc::Receiver<super::OutboundWebSocketEvent>,
) {
    let (sender, receiver) = mpsc::channel(capacity);
    (
        WebSocketConnectionHandle {
            connection_id,
            sender,
        },
        receiver,
    )
}

struct EchoHandler;

impl WebSocketMessageHandler for EchoHandler {
    type Error = std::convert::Infallible;
    type HandleFuture<'a> = std::future::Ready<Result<WebSocketHandlerAction, Self::Error>>;

    fn on_message<'a>(
        &'a mut self,
        _context: WebSocketConnectionContext,
        message: WebSocketApplicationMessage,
        outbound: &'a super::WebSocketConnectionHandle,
    ) -> Self::HandleFuture<'a> {
        match message {
            WebSocketApplicationMessage::Text(text) => {
                let _ = outbound.try_send_message(super::WebSocketMessage::Text(text));
            }
            WebSocketApplicationMessage::Binary(binary) => {
                let _ = outbound.try_send_message(super::WebSocketMessage::Binary(binary));
            }
        }

        std::future::ready(Ok(WebSocketHandlerAction::Continue))
    }
}

struct PendingHandler;

impl WebSocketMessageHandler for PendingHandler {
    type Error = std::convert::Infallible;
    type HandleFuture<'a> = std::future::Pending<Result<WebSocketHandlerAction, Self::Error>>;

    fn on_message<'a>(
        &'a mut self,
        _context: WebSocketConnectionContext,
        _message: WebSocketApplicationMessage,
        _outbound: &'a super::WebSocketConnectionHandle,
    ) -> Self::HandleFuture<'a> {
        std::future::pending()
    }
}

async fn bind_websocket_test_listener() -> tokio::net::TcpListener {
    tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("websocket test listener should bind")
}

#[derive(Clone)]
struct WebSocketTestState {
    runtime: WebSocketConnectionRuntime,
}

async fn websocket_route(
    ws: WebSocketUpgrade,
    State(state): State<WebSocketTestState>,
) -> Response {
    websocket_upgrade_response(
        ws,
        WebSocketConnectionContext::new(None, None),
        state.runtime.clone(),
        EchoHandler,
    )
}

use axum::response::Response;

#[test]
fn connection_id_generation_is_unique() {
    let first = ConnectionId::generate();
    let second = ConnectionId::generate();

    assert_ne!(first, second);
}

#[test]
fn websocket_decoder_errors_map_to_protocol_close_codes() {
    use tungstenite::error::{CapacityError, Error, ProtocolError};

    let oversized = axum::Error::new(Error::Capacity(CapacityError::MessageTooLong {
        size: 65,
        max_size: 64,
    }));
    assert_eq!(
        websocket_protocol_close_reason(&oversized).map(|reason| reason.code()),
        Some(1009)
    );

    let invalid_utf8 = axum::Error::new(Error::Utf8("invalid utf8".to_owned()));
    assert_eq!(
        websocket_protocol_close_reason(&invalid_utf8).map(|reason| reason.code()),
        Some(1007)
    );

    let unmasked = axum::Error::new(Error::Protocol(ProtocolError::UnmaskedFrameFromClient));
    assert_eq!(
        websocket_protocol_close_reason(&unmasked).map(|reason| reason.code()),
        Some(1002)
    );
}

#[test]
fn configure_websocket_upgrade_applies_limits() {
    let _function: fn(WebSocketUpgrade, WebSocketLimits) -> WebSocketUpgrade =
        configure_websocket_upgrade;
}

#[test]
fn bounded_outbound_channel_reports_full_capacity() {
    let (handle, _receiver) = make_outbound_channel(
        ConnectionId::generate(),
        OutboundQueueCapacity::new(1)
            .expect("fixture should be valid")
            .as_usize(),
    );

    handle
        .try_send_message(super::WebSocketMessage::Text("first".to_owned().into()))
        .expect("first message should fit in bounded queue");

    let result = handle.try_send_message(super::WebSocketMessage::Text("second".to_owned().into()));

    assert_eq!(
        result,
        Err(WebSocketSendError::new(
            WebSocketSendErrorReason::OutboundQueueFull
        ))
    );
}

#[test]
fn application_message_debug_redacts_payloads() {
    let text_debug = format!(
        "{:?}",
        WebSocketApplicationMessage::Text("secret-message".into()).into_owned()
    );
    let binary_debug = format!(
        "{:?}",
        WebSocketApplicationMessage::Binary(vec![1, 2, 3].into()).into_owned()
    );

    assert!(text_debug.contains("<redacted>"));
    assert!(text_debug.contains("byte_len"));
    assert!(!text_debug.contains("secret-message"));
    assert!(binary_debug.contains("<redacted>"));
    assert!(!binary_debug.contains("[1, 2, 3]"));
}

#[test]
fn websocket_connection_limiter_enforces_active_connection_capacity() {
    let limiter = WebSocketConnectionLimiter::new(
        RuntimeConcurrencyLimit::new(1, ConcurrencyLimitConfigField::WebSocketConnections)
            .expect("fixture should be valid"),
    );
    let first_permit = limiter
        .try_acquire()
        .expect("first connection should fit within capacity");

    let second_result = limiter.try_acquire();
    assert!(matches!(
        second_result,
        Err(error) if error == WebSocketConnectionLimitError::new(
            WebSocketConnectionLimitErrorReason::TooManyActiveConnections,
        )
    ));

    drop(first_permit);

    assert!(limiter.try_acquire().is_ok());
}

#[test]
fn websocket_connection_limiter_does_not_share_a_source_bucket_for_unknown_peers() {
    let limiter = WebSocketConnectionLimiter::new(
        RuntimeConcurrencyLimit::new(128, ConcurrencyLimitConfigField::WebSocketConnections)
            .expect("fixture should be valid"),
    );
    let mut permits = Vec::new();
    for _ in 0..65 {
        permits.push(
            limiter
                .try_acquire()
                .expect("unknown peers are bounded by global capacity"),
        );
    }
    assert_eq!(permits.len(), 65);
}

#[test]
fn websocket_connection_limiter_preserves_capacity_for_other_sources() {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    let limiter = WebSocketConnectionLimiter::new(
        RuntimeConcurrencyLimit::new(128, ConcurrencyLimitConfigField::WebSocketConnections)
            .expect("fixture should be valid"),
    );
    let first_source = Some(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, 1)));
    let mut permits = Vec::new();
    for suffix in 1..=64 {
        let source = Some(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0xdb8, 1, 2, 0, 0, 0, suffix,
        )));
        permits.push(
            limiter
                .try_acquire_for_source(source)
                .expect("source should fit within its per-network budget"),
        );
    }
    assert!(matches!(
        limiter.try_acquire_for_source(first_source),
        Err(error) if error.reason() == WebSocketConnectionLimitErrorReason::TooManyActiveConnections
    ));
    assert!(
        limiter
            .try_acquire_for_source(Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))))
            .is_ok()
    );

    drop(permits);
    assert!(limiter.try_acquire_for_source(first_source).is_ok());
}

#[tokio::test]
async fn application_handler_wait_is_shutdown_cancellable() {
    let controller = ShutdownController::new();
    let mut shutdown = controller.token();
    let context = WebSocketConnectionContext::new(None, None);
    let (outbound, mut receiver) = make_outbound_channel(context.connection_id(), 1);
    let mut handler = PendingHandler;
    let mut socket = futures_util::sink::drain();

    assert!(controller.begin_shutdown(crate::shutdown::ShutdownReason::Sigterm));

    let result = handle_application_message(
        &mut handler,
        ApplicationMessageInput::new(context, WebSocketApplicationMessage::Text("hello".into())),
        &outbound,
        &mut receiver,
        &mut socket,
        &mut shutdown,
        Duration::from_secs(30),
    )
    .await;

    assert_eq!(result, ApplicationMessageOutcome::ShutdownRequested);
}

#[tokio::test]
async fn application_handler_has_a_bounded_deadline() {
    let controller = ShutdownController::new();
    let mut shutdown = controller.token();
    let context = WebSocketConnectionContext::new(None, None);
    let (outbound, mut receiver) = make_outbound_channel(context.connection_id(), 1);
    let mut handler = PendingHandler;
    let mut socket = futures_util::sink::drain();

    let result = handle_application_message(
        &mut handler,
        ApplicationMessageInput::new(context, WebSocketApplicationMessage::Text("hello".into())),
        &outbound,
        &mut receiver,
        &mut socket,
        &mut shutdown,
        Duration::from_millis(10),
    )
    .await;

    assert_eq!(result, ApplicationMessageOutcome::HandlerTimeout);
}

struct ThreeRepliesHandler;

impl WebSocketMessageHandler for ThreeRepliesHandler {
    type Error = WebSocketSendError;
    type HandleFuture<'a> = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<WebSocketHandlerAction, Self::Error>>
                + Send
                + 'a,
        >,
    >;

    fn on_message<'a>(
        &'a mut self,
        _context: WebSocketConnectionContext,
        _message: WebSocketApplicationMessage,
        outbound: &'a WebSocketConnectionHandle,
    ) -> Self::HandleFuture<'a> {
        Box::pin(async move {
            for _ in 0..3 {
                outbound
                    .send_message(super::WebSocketMessage::Text("reply".into()))
                    .await?;
            }
            Ok(WebSocketHandlerAction::Continue)
        })
    }
}

#[tokio::test]
async fn handler_can_enqueue_more_replies_than_outbound_capacity() {
    let controller = ShutdownController::new();
    let mut shutdown = controller.token();
    let context = WebSocketConnectionContext::new(None, None);
    let (outbound, mut receiver) = make_outbound_channel(context.connection_id(), 1);
    let mut handler = ThreeRepliesHandler;
    let mut socket = futures_util::sink::drain();

    let outcome = tokio::time::timeout(
        Duration::from_secs(2),
        handle_application_message(
            &mut handler,
            ApplicationMessageInput::new(
                context,
                WebSocketApplicationMessage::Text("request".into()),
            ),
            &outbound,
            &mut receiver,
            &mut socket,
            &mut shutdown,
            Duration::from_secs(30),
        ),
    )
    .await
    .expect("handler should not wait on its own queue");
    assert_eq!(outcome, ApplicationMessageOutcome::Continue);
}

#[tokio::test]
async fn websocket_route_can_echo_without_protocol_assumptions() {
    let shutdown = ShutdownController::new();
    let app = Router::new()
        .route("/ws", get(websocket_route))
        .with_state(WebSocketTestState {
            runtime: WebSocketConnectionRuntime::new(
                WebSocketLimits::safe_defaults(),
                WebSocketHeartbeatConfig::new(
                    HeartbeatInterval::new(Duration::from_secs(30))
                        .expect("fixture should be valid"),
                    IdleTimeout::new(Duration::from_secs(90)).expect("fixture should be valid"),
                )
                .expect("fixture should be valid"),
                WebSocketShutdownConfig::safe_defaults(),
                shutdown.token(),
                Arc::new(NoopWebSocketConnectionHooks),
                WebSocketConnectionLimiter::new(
                    RuntimeConcurrencyLimit::new(
                        1,
                        ConcurrencyLimitConfigField::WebSocketConnections,
                    )
                    .expect("fixture should be valid"),
                ),
            ),
        });
    let listener = bind_websocket_test_listener().await;
    let address = listener
        .local_addr()
        .expect("websocket test listener should expose a local address");
    let (shutdown_sender, shutdown_receiver) = oneshot::channel::<()>();
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
        .send(Message::Text("hello".into()))
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
        "hello"
    );

    websocket
        .send(Message::Close(None))
        .await
        .expect("websocket test client should initiate close");
    let close_reply = tokio::time::timeout(Duration::from_secs(2), websocket.next())
        .await
        .expect("websocket close reply should arrive before timeout")
        .expect("websocket should send a close reply")
        .expect("websocket close reply should be a valid frame");
    assert!(matches!(close_reply, Message::Close(_)));

    shutdown_sender
        .send(())
        .expect("websocket test server shutdown receiver should be alive");
    server_task
        .await
        .expect("websocket test server task should join cleanly");
}

#[test]
fn websocket_runtime_connection_limit_is_shared_between_runtime_instances() {
    let shutdown = ShutdownController::new();
    let limiter = WebSocketConnectionLimiter::new(
        RuntimeConcurrencyLimit::new(1, ConcurrencyLimitConfigField::WebSocketConnections)
            .expect("fixture should be valid"),
    );
    let first = WebSocketConnectionRuntime::new(
        WebSocketLimits::safe_defaults(),
        WebSocketHeartbeatConfig::safe_defaults(),
        WebSocketShutdownConfig::safe_defaults(),
        shutdown.token(),
        Arc::new(NoopWebSocketConnectionHooks),
        limiter.clone(),
    );
    let second = WebSocketConnectionRuntime::new(
        WebSocketLimits::safe_defaults(),
        WebSocketHeartbeatConfig::safe_defaults(),
        WebSocketShutdownConfig::safe_defaults(),
        shutdown.token(),
        Arc::new(NoopWebSocketConnectionHooks),
        limiter,
    );

    let first_permit = first
        .try_acquire_connection_permit(None)
        .expect("first runtime should acquire the sole permit while limit is one");
    assert!(
        second.try_acquire_connection_permit(None).is_err(),
        "second runtime should enforce shared limiter state and be capped",
    );

    drop(first_permit);

    assert!(
        second.try_acquire_connection_permit(None).is_ok(),
        "permit should become available after first runtime releases it",
    );
}
