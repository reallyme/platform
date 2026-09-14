// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{JetStreamConsumerBackend, JetStreamDelivery, JetStreamPullConsumer};

impl std::fmt::Debug for JetStreamDelivery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamDelivery")
            .field("subject", &self.subject)
            .field("payload_len", &self.payload.len())
            .field("has_headers", &self.headers.is_some())
            .field("stream_sequence", &self.info.stream_sequence)
            .field("consumer_sequence", &self.info.consumer_sequence)
            .field("pending", &self.info.pending)
            .finish()
    }
}

impl<B> std::fmt::Debug for JetStreamPullConsumer<B>
where
    B: JetStreamConsumerBackend,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JetStreamPullConsumer")
            .field("enabled", &self.config.enabled())
            .field("stream_name", &self.config.stream_name())
            .field("consumer_name", &self.config.consumer_name())
            .field("subject", &self.config.subject())
            .field(
                "operation_timeout_millis",
                &self.config.operation_timeout().as_millis(),
            )
            .field("ack_timeout_millis", &self.config.ack_timeout().as_millis())
            .field("max_ack_pending", &self.config.max_ack_pending())
            .finish()
    }
}
