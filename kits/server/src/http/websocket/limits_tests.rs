// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    DEFAULT_INBOUND_FRAME_SIZE_BYTES, DEFAULT_INBOUND_MESSAGE_SIZE_BYTES,
    DEFAULT_OUTBOUND_QUEUE_CAPACITY, InboundFrameSizeBytes, InboundMessageSizeBytes,
    MAX_INBOUND_FRAME_SIZE_BYTES, MAX_INBOUND_MESSAGE_SIZE_BYTES, MAX_OUTBOUND_QUEUE_CAPACITY,
    OutboundQueueCapacity, WebSocketLimits,
};
use crate::http::websocket::{
    WebSocketConfigError, WebSocketLimitField, WebSocketValidationErrorReason,
};

#[test]
fn rejects_zero_limits() {
    assert_eq!(
        InboundMessageSizeBytes::new(0),
        Err(WebSocketConfigError::InvalidLimits {
            field: WebSocketLimitField::MaxMessageSizeBytes,
            reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
        })
    );
    assert_eq!(
        OutboundQueueCapacity::new(0),
        Err(WebSocketConfigError::InvalidLimits {
            field: WebSocketLimitField::OutboundQueueCapacity,
            reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
        })
    );
}

#[test]
fn rejects_limits_above_platform_maximums() {
    assert_eq!(
        InboundMessageSizeBytes::new(MAX_INBOUND_MESSAGE_SIZE_BYTES + 1),
        Err(WebSocketConfigError::InvalidLimits {
            field: WebSocketLimitField::MaxMessageSizeBytes,
            reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        })
    );
    assert_eq!(
        InboundFrameSizeBytes::new(MAX_INBOUND_FRAME_SIZE_BYTES + 1),
        Err(WebSocketConfigError::InvalidLimits {
            field: WebSocketLimitField::MaxFrameSizeBytes,
            reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        })
    );
    assert_eq!(
        OutboundQueueCapacity::new(MAX_OUTBOUND_QUEUE_CAPACITY + 1),
        Err(WebSocketConfigError::InvalidLimits {
            field: WebSocketLimitField::OutboundQueueCapacity,
            reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
        })
    );
}

#[test]
fn accepts_limit_boundary_values() {
    assert!(InboundMessageSizeBytes::new(MAX_INBOUND_MESSAGE_SIZE_BYTES).is_ok());
    assert!(InboundFrameSizeBytes::new(MAX_INBOUND_FRAME_SIZE_BYTES).is_ok());
    assert!(OutboundQueueCapacity::new(MAX_OUTBOUND_QUEUE_CAPACITY).is_ok());
}

#[test]
fn rejects_frame_size_larger_than_message_size() {
    let result = WebSocketLimits::new(
        InboundMessageSizeBytes::new(64).expect("fixture should be valid"),
        InboundFrameSizeBytes::new(128).expect("fixture should be valid"),
        OutboundQueueCapacity::new(4).expect("fixture should be valid"),
    );

    assert_eq!(
        result,
        Err(WebSocketConfigError::InvalidLimits {
            field: WebSocketLimitField::MaxFrameSizeBytes,
            reason: WebSocketValidationErrorReason::MaxFrameSizeMustNotExceedMaxMessageSize,
        })
    );
}

#[test]
fn safe_defaults_are_conservative_and_valid() {
    let limits = WebSocketLimits::safe_defaults();

    assert_eq!(
        limits.max_message_size().as_usize(),
        DEFAULT_INBOUND_MESSAGE_SIZE_BYTES
    );
    assert_eq!(
        limits.max_frame_size().as_usize(),
        DEFAULT_INBOUND_FRAME_SIZE_BYTES
    );
    assert_eq!(
        limits.outbound_queue_capacity().as_usize(),
        DEFAULT_OUTBOUND_QUEUE_CAPACITY
    );
}
