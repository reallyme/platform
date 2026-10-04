// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Maps protocol errors to close frames without exposing transport details.

use std::error::Error as _;

use tungstenite::error::{CapacityError, Error};

use super::WebSocketCloseReason;

pub(super) fn websocket_protocol_close_reason(error: &axum::Error) -> Option<WebSocketCloseReason> {
    let cause = error.source()?.downcast_ref::<Error>()?;
    match cause {
        Error::Capacity(CapacityError::MessageTooLong { .. }) => {
            Some(WebSocketCloseReason::MessageTooLarge)
        }
        Error::Utf8(_) => Some(WebSocketCloseReason::InvalidPayload),
        Error::Protocol(_) | Error::AttackAttempt => Some(WebSocketCloseReason::ProtocolError),
        _ => None,
    }
}
