// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::WebSocketCloseReason;

#[test]
fn close_reason_mapping_is_stable() {
    assert_eq!(WebSocketCloseReason::NormalClosure.code(), 1000);
    assert_eq!(WebSocketCloseReason::IdleTimeout.code(), 1001);
    assert_eq!(WebSocketCloseReason::MessageTooLarge.code(), 1009);
    assert_eq!(WebSocketCloseReason::InvalidMessage.code(), 1008);
    assert_eq!(
        WebSocketCloseReason::ServerShutdown.to_close_frame().reason,
        "server shutdown"
    );
}
