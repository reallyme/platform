// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
    let capacity = TaskChannelCapacity::new(1).expect("channel capacity fixture should be valid");
    let (sender, _receiver) = bounded_task_channel::<u8>(capacity);

    assert_eq!(sender.try_send(1), Ok(()));
    assert!(matches!(sender.try_send(2), Err(TrySendError::Full(2))));
}

#[test]
fn try_send_bounded_task_reports_full_capacity() {
    let capacity = TaskChannelCapacity::new(1).expect("channel capacity fixture should be valid");
    let (sender, _receiver) = bounded_task_channel::<u8>(capacity);

    assert_eq!(try_send_bounded_task(&sender, 1), Ok(()));
    assert!(matches!(
        try_send_bounded_task(&sender, 2),
        Err(TrySendError::Full(2))
    ));
}
