// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use tokio::sync::mpsc;

use super::error::{TaskChannelError, TaskChannelValidationErrorReason};
#[cfg(feature = "metrics")]
use crate::observability::{RuntimeQueueLabel, record_runtime_queue_saturation};

/// Validated capacity for bounded background-task channels.
///
/// Service code should prefer explicit bounded channels for coordination with
/// long-lived tasks so queue growth remains reviewable and memory usage stays
/// bounded under backpressure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskChannelCapacity(usize);

impl TaskChannelCapacity {
    /// Constructs a validated bounded-channel capacity.
    pub fn new(value: usize) -> Result<Self, TaskChannelError> {
        if value == 0 {
            return Err(TaskChannelError::new(
                TaskChannelValidationErrorReason::MustBeGreaterThanZero,
            ));
        }

        Ok(Self(value))
    }

    /// Returns the validated capacity as a plain `usize`.
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

/// Creates a bounded Tokio MPSC channel for managed background-task workflows.
pub fn bounded_task_channel<T>(
    capacity: TaskChannelCapacity,
) -> (mpsc::Sender<T>, mpsc::Receiver<T>) {
    mpsc::channel(capacity.as_usize())
}

/// Attempts to enqueue one item into a bounded task channel and records
/// aggregate queue-saturation telemetry when the queue is full.
///
/// This helper exists so app code can preserve explicit backpressure behavior
/// without hand-rolling metrics at each call site. It intentionally does not
/// log on saturation because a hot full queue can otherwise produce incident
/// noise.
pub fn try_send_bounded_task<T>(
    sender: &mpsc::Sender<T>,
    value: T,
) -> Result<(), mpsc::error::TrySendError<T>> {
    match sender.try_send(value) {
        Ok(()) => Ok(()),
        Err(error @ mpsc::error::TrySendError::Full(_)) => {
            #[cfg(feature = "metrics")]
            record_runtime_queue_saturation(RuntimeQueueLabel::TaskChannel);
            Err(error)
        }
        Err(error @ mpsc::error::TrySendError::Closed(_)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::error::TrySendError;

    use super::{TaskChannelCapacity, bounded_task_channel, try_send_bounded_task};
    use crate::task::{TaskChannelError, TaskChannelValidationErrorReason};

    #[test]
    fn rejects_zero_channel_capacity() {
        assert_eq!(
            TaskChannelCapacity::new(0),
            Err(TaskChannelError::new(
                TaskChannelValidationErrorReason::MustBeGreaterThanZero
            ))
        );
    }

    #[test]
    fn bounded_channel_reports_full_capacity() {
        let capacity =
            TaskChannelCapacity::new(1).expect("channel capacity fixture should be valid");
        let (sender, _receiver) = bounded_task_channel::<u8>(capacity);

        assert_eq!(sender.try_send(1), Ok(()));
        assert!(matches!(sender.try_send(2), Err(TrySendError::Full(2))));
    }

    #[test]
    fn try_send_bounded_task_reports_full_capacity() {
        let capacity =
            TaskChannelCapacity::new(1).expect("channel capacity fixture should be valid");
        let (sender, _receiver) = bounded_task_channel::<u8>(capacity);

        assert_eq!(try_send_bounded_task(&sender, 1), Ok(()));
        assert!(matches!(
            try_send_bounded_task(&sender, 2),
            Err(TrySendError::Full(2))
        ));
    }
}
