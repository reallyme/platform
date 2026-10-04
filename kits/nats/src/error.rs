// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Low-cardinality JetStream failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JetStreamErrorReason {
    /// The provided configuration was invalid or incomplete.
    InvalidConfiguration,
    /// The feature is disabled by configuration.
    Disabled,
    /// Connecting to NATS failed.
    ConnectFailed,
    /// Looking up the configured stream failed.
    StreamLookupFailed,
    /// Creating or retrieving the configured consumer failed.
    ConsumerInitializationFailed,
    /// An existing durable consumer has a different delivery policy.
    ConsumerConfigurationMismatch,
    /// The publish payload exceeded the configured bound.
    PayloadTooLarge,
    /// A caller-supplied deduplication header value was unsafe.
    InvalidMessageId,
    /// A required synchronization primitive was poisoned.
    SyncPrimitivePoisoned,
    /// JetStream did not acknowledge the publish.
    PublishNotAcknowledged,
    /// Pulling messages from JetStream failed.
    PullFailed,
    /// Acknowledging a consumed JetStream message failed.
    AcknowledgmentFailed,
}

/// Typed JetStream infrastructure error.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum JetStreamError {
    /// The provided configuration was invalid or incomplete.
    #[error("jetstream configuration is invalid")]
    InvalidConfiguration,
    /// The feature is disabled by configuration.
    #[error("jetstream feature is disabled")]
    Disabled,
    /// Connecting to NATS failed.
    #[error("jetstream nats connection failed")]
    ConnectFailed,
    /// Looking up the configured stream failed.
    #[error("jetstream stream lookup failed")]
    StreamLookupFailed,
    /// Creating or retrieving the configured consumer failed.
    #[error("jetstream consumer initialization failed")]
    ConsumerInitializationFailed,
    /// An existing durable consumer has a different delivery policy.
    #[error("jetstream durable consumer configuration differs from the requested policy")]
    ConsumerConfigurationMismatch,
    /// The publish payload exceeded the configured bound.
    #[error("jetstream publish payload exceeds the configured bound")]
    PayloadTooLarge,
    /// A caller-supplied deduplication header value was unsafe.
    #[error("jetstream message id is invalid")]
    InvalidMessageId,
    /// A required synchronization primitive was poisoned.
    #[error("jetstream mutex or lock is poisoned")]
    SyncPrimitivePoisoned,
    /// JetStream did not acknowledge the publish.
    #[error("jetstream publish was not acknowledged")]
    PublishNotAcknowledged,
    /// Pulling messages from JetStream failed.
    #[error("jetstream pull failed")]
    PullFailed,
    /// Acknowledging a consumed JetStream message failed.
    #[error("jetstream acknowledgment failed")]
    AcknowledgmentFailed,
}

impl JetStreamError {
    /// Returns the stable low-cardinality failure reason.
    pub const fn reason(self) -> JetStreamErrorReason {
        match self {
            Self::InvalidConfiguration => JetStreamErrorReason::InvalidConfiguration,
            Self::Disabled => JetStreamErrorReason::Disabled,
            Self::ConnectFailed => JetStreamErrorReason::ConnectFailed,
            Self::StreamLookupFailed => JetStreamErrorReason::StreamLookupFailed,
            Self::ConsumerInitializationFailed => {
                JetStreamErrorReason::ConsumerInitializationFailed
            }
            Self::ConsumerConfigurationMismatch => {
                JetStreamErrorReason::ConsumerConfigurationMismatch
            }
            Self::PayloadTooLarge => JetStreamErrorReason::PayloadTooLarge,
            Self::InvalidMessageId => JetStreamErrorReason::InvalidMessageId,
            Self::SyncPrimitivePoisoned => JetStreamErrorReason::SyncPrimitivePoisoned,
            Self::PublishNotAcknowledged => JetStreamErrorReason::PublishNotAcknowledged,
            Self::PullFailed => JetStreamErrorReason::PullFailed,
            Self::AcknowledgmentFailed => JetStreamErrorReason::AcknowledgmentFailed,
        }
    }
}
