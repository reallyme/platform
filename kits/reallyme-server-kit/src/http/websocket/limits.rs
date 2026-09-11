// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{WebSocketConfigError, WebSocketLimitField, WebSocketValidationErrorReason};

/// Conservative default maximum inbound message size in bytes.
pub const DEFAULT_INBOUND_MESSAGE_SIZE_BYTES: usize = 64 * 1024;
/// Conservative default maximum inbound frame size in bytes.
pub const DEFAULT_INBOUND_FRAME_SIZE_BYTES: usize = 16 * 1024;
/// Conservative default bounded outbound queue capacity.
pub const DEFAULT_OUTBOUND_QUEUE_CAPACITY: usize = 32;
/// Platform maximum inbound message size in bytes.
///
/// This hard ceiling prevents a service misconfiguration from allowing a
/// single WebSocket connection to request arbitrarily large message buffers.
pub const MAX_INBOUND_MESSAGE_SIZE_BYTES: usize = 16 * 1024 * 1024;
/// Platform maximum inbound frame size in bytes.
///
/// Frames are capped separately so fragmented-message handling cannot be
/// configured into unexpectedly large per-frame allocations.
pub const MAX_INBOUND_FRAME_SIZE_BYTES: usize = 4 * 1024 * 1024;
/// Platform maximum bounded outbound queue capacity per connection.
///
/// Outbound queues are per connection, so even bounded queues need a platform
/// maximum to avoid multiplying excessive memory by connection count.
pub const MAX_OUTBOUND_QUEUE_CAPACITY: usize = 1024;

/// Validated inbound WebSocket message size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboundMessageSizeBytes(usize);

impl InboundMessageSizeBytes {
    /// Creates a validated inbound message size limit.
    pub fn new(value: usize) -> Result<Self, WebSocketConfigError> {
        if value == 0 {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::MaxMessageSizeBytes,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value > MAX_INBOUND_MESSAGE_SIZE_BYTES {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::MaxMessageSizeBytes,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the limit in bytes.
    pub const fn as_usize(self) -> usize {
        self.0
    }

    fn safe_default() -> Self {
        Self(DEFAULT_INBOUND_MESSAGE_SIZE_BYTES)
    }
}

/// Validated inbound WebSocket frame size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InboundFrameSizeBytes(usize);

impl InboundFrameSizeBytes {
    /// Creates a validated inbound frame size limit.
    pub fn new(value: usize) -> Result<Self, WebSocketConfigError> {
        if value == 0 {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::MaxFrameSizeBytes,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value > MAX_INBOUND_FRAME_SIZE_BYTES {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::MaxFrameSizeBytes,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the limit in bytes.
    pub const fn as_usize(self) -> usize {
        self.0
    }

    fn safe_default() -> Self {
        Self(DEFAULT_INBOUND_FRAME_SIZE_BYTES)
    }
}

/// Validated bounded outbound queue capacity for a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboundQueueCapacity(usize);

impl OutboundQueueCapacity {
    /// Creates a validated outbound queue capacity.
    pub fn new(value: usize) -> Result<Self, WebSocketConfigError> {
        if value == 0 {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::OutboundQueueCapacity,
                reason: WebSocketValidationErrorReason::MustBeGreaterThanZero,
            });
        }

        if value > MAX_OUTBOUND_QUEUE_CAPACITY {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::OutboundQueueCapacity,
                reason: WebSocketValidationErrorReason::MustBeLessThanOrEqualToMaximum,
            });
        }

        Ok(Self(value))
    }

    /// Returns the bounded queue capacity.
    pub const fn as_usize(self) -> usize {
        self.0
    }

    fn safe_default() -> Self {
        Self(DEFAULT_OUTBOUND_QUEUE_CAPACITY)
    }
}

/// Validated WebSocket connection limits.
///
/// These limits bound inbound message parsing at the upgrade layer and bound
/// the runtime's outbound message queue so connections do not accumulate
/// unbounded transport buffers under backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebSocketLimits {
    max_message_size: InboundMessageSizeBytes,
    max_frame_size: InboundFrameSizeBytes,
    outbound_queue_capacity: OutboundQueueCapacity,
}

impl WebSocketLimits {
    /// Constructs validated WebSocket limits.
    pub fn new(
        max_message_size: InboundMessageSizeBytes,
        max_frame_size: InboundFrameSizeBytes,
        outbound_queue_capacity: OutboundQueueCapacity,
    ) -> Result<Self, WebSocketConfigError> {
        if max_frame_size.as_usize() > max_message_size.as_usize() {
            return Err(WebSocketConfigError::InvalidLimits {
                field: WebSocketLimitField::MaxFrameSizeBytes,
                reason: WebSocketValidationErrorReason::MaxFrameSizeMustNotExceedMaxMessageSize,
            });
        }

        Ok(Self {
            max_message_size,
            max_frame_size,
            outbound_queue_capacity,
        })
    }

    /// Returns a conservative safe default limit set.
    pub fn safe_defaults() -> Self {
        Self {
            max_message_size: InboundMessageSizeBytes::safe_default(),
            max_frame_size: InboundFrameSizeBytes::safe_default(),
            outbound_queue_capacity: OutboundQueueCapacity::safe_default(),
        }
    }

    /// Returns the maximum inbound message size.
    pub const fn max_message_size(self) -> InboundMessageSizeBytes {
        self.max_message_size
    }

    /// Returns the maximum inbound frame size.
    pub const fn max_frame_size(self) -> InboundFrameSizeBytes {
        self.max_frame_size
    }

    /// Returns the bounded outbound queue capacity.
    pub const fn outbound_queue_capacity(self) -> OutboundQueueCapacity {
        self.outbound_queue_capacity
    }
}

impl Default for WebSocketLimits {
    fn default() -> Self {
        Self::safe_defaults()
    }
}

#[cfg(test)]
mod tests {
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
}
