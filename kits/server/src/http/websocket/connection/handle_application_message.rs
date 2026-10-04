// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Runs one ordered application handler while draining its outbound queue.

use futures_util::{Sink, SinkExt};
use tokio::sync::mpsc;

use super::{
    ApplicationMessageOutcome, OutboundWebSocketEvent, WebSocketApplicationMessage,
    WebSocketConnectionContext, WebSocketConnectionHandle, WebSocketHandlerAction,
    WebSocketMessage, WebSocketMessageHandler,
};
use crate::task::ShutdownToken;

pub(super) async fn handle_application_message<Handler, Socket>(
    handler: &mut Handler,
    context: WebSocketConnectionContext,
    message: WebSocketApplicationMessage,
    outbound: &WebSocketConnectionHandle,
    receiver: &mut mpsc::Receiver<OutboundWebSocketEvent>,
    socket: &mut Socket,
    shutdown: &mut ShutdownToken,
) -> ApplicationMessageOutcome
where
    Handler: WebSocketMessageHandler,
    Socket: Sink<WebSocketMessage> + Unpin,
{
    let handling = handler.on_message(context, message, outbound);
    tokio::pin!(handling);

    loop {
        tokio::select! {
            result = &mut handling => {
                return match result {
                    Ok(WebSocketHandlerAction::Continue) => ApplicationMessageOutcome::Continue,
                    Ok(WebSocketHandlerAction::Close(reason)) => ApplicationMessageOutcome::Close(reason),
                    Err(_) => ApplicationMessageOutcome::HandlerError,
                };
            }
            event = receiver.recv() => {
                match event {
                    Some(OutboundWebSocketEvent::Message(message)) => {
                        if !matches!(
                            tokio::time::timeout(
                                super::SOCKET_WRITE_TIMEOUT,
                                socket.send(message),
                            )
                            .await,
                            Ok(Ok(()))
                        ) {
                            return ApplicationMessageOutcome::TransportError;
                        }
                    }
                    Some(OutboundWebSocketEvent::Close(reason)) => {
                        return ApplicationMessageOutcome::Close(reason);
                    }
                    None => return ApplicationMessageOutcome::TransportError,
                }
            }
            _ = shutdown.cancelled() => return ApplicationMessageOutcome::ShutdownRequested,
        }
    }
}
