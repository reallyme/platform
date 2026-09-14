// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use metrics::{counter, describe_counter};

use super::connection::WebSocketConnectionOutcome;

const METRIC_LABEL_OUTCOME: &str = "outcome";
const METRIC_LABEL_REASON: &str = "reason";

const METRIC_NAME_CONNECTIONS_OPENED_TOTAL: &str = "reallyme_websocket_connections_opened_total";
const METRIC_NAME_CONNECTIONS_CLOSED_TOTAL: &str = "reallyme_websocket_connections_closed_total";
const METRIC_NAME_CONNECTION_ERRORS_TOTAL: &str = "reallyme_websocket_connection_errors_total";
const METRIC_NAME_CONNECTION_TIMEOUTS_TOTAL: &str = "reallyme_websocket_connection_timeouts_total";

/// Low-cardinality connection close outcome label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketConnectionOutcomeLabel {
    /// The peer closed the socket.
    PeerClosed,
    /// The server closed the socket intentionally.
    ServerClosed,
    /// The handler failed internally.
    HandlerError,
    /// The transport failed unexpectedly.
    TransportError,
}

impl WebSocketConnectionOutcomeLabel {
    /// Returns the stable label value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PeerClosed => "peer_closed",
            Self::ServerClosed => "server_closed",
            Self::HandlerError => "handler_error",
            Self::TransportError => "transport_error",
        }
    }
}

/// Low-cardinality connection error label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSocketConnectionErrorLabel {
    /// The user handler failed.
    HandlerError,
    /// The underlying socket transport failed.
    TransportError,
}

impl WebSocketConnectionErrorLabel {
    /// Returns the stable label value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HandlerError => "handler_error",
            Self::TransportError => "transport_error",
        }
    }
}

/// Describes the standard WebSocket metrics emitted by the server kit.
pub fn describe_websocket_metrics() {
    describe_counter!(
        METRIC_NAME_CONNECTIONS_OPENED_TOTAL,
        "Total opened WebSocket connections."
    );
    describe_counter!(
        METRIC_NAME_CONNECTIONS_CLOSED_TOTAL,
        "Total closed WebSocket connections by low-cardinality outcome."
    );
    describe_counter!(
        METRIC_NAME_CONNECTION_ERRORS_TOTAL,
        "Total WebSocket connection errors by low-cardinality reason."
    );
    describe_counter!(
        METRIC_NAME_CONNECTION_TIMEOUTS_TOTAL,
        "Total WebSocket connections closed by server-side idle timeout."
    );
}

/// Records an opened WebSocket connection.
pub fn record_websocket_connection_opened() {
    counter!(METRIC_NAME_CONNECTIONS_OPENED_TOTAL).increment(1);
}

/// Records a closed WebSocket connection.
pub fn record_websocket_connection_closed(outcome: WebSocketConnectionOutcome) {
    counter!(
        METRIC_NAME_CONNECTIONS_CLOSED_TOTAL,
        METRIC_LABEL_OUTCOME => outcome.metric_label().as_str()
    )
    .increment(1);
}

/// Records a WebSocket connection error.
pub fn record_websocket_connection_error(reason: WebSocketConnectionErrorLabel) {
    counter!(
        METRIC_NAME_CONNECTION_ERRORS_TOTAL,
        METRIC_LABEL_REASON => reason.as_str()
    )
    .increment(1);
}

/// Records a WebSocket connection closed due to server-side idle timeout.
pub fn record_websocket_connection_timeout() {
    counter!(METRIC_NAME_CONNECTION_TIMEOUTS_TOTAL).increment(1);
}
