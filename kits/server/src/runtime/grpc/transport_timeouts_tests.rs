// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{GrpcTransportTimeouts, GrpcTransportTimeoutsErrorReason};
use std::time::Duration;

#[test]
fn transport_timeouts_keep_a_finite_post_goaway_grace() {
    let configured = GrpcTransportTimeouts::new(
        Duration::from_secs(20),
        Duration::from_secs(5),
        Duration::from_secs(60),
        Duration::from_secs(180),
        Duration::from_secs(90),
        Duration::from_secs(3),
    )
    .expect("valid fixture");
    assert_eq!(configured.hard_cap(), Duration::from_secs(240));
    assert_eq!(configured.idle_timeout(), Duration::from_secs(90));
    assert_eq!(configured.first_request_timeout(), Duration::from_secs(3));
}

#[test]
fn zero_or_overflowing_transport_deadline_is_rejected() {
    let defaults = GrpcTransportTimeouts::default();
    assert_eq!(
        GrpcTransportTimeouts::new(
            Duration::ZERO,
            defaults.keepalive_timeout(),
            defaults.max_age(),
            Duration::from_secs(1),
            defaults.idle_timeout(),
            defaults.first_request_timeout(),
        )
        .expect_err("zero timeout rejected")
        .reason(),
        GrpcTransportTimeoutsErrorReason::Zero
    );
    assert_eq!(
        GrpcTransportTimeouts::new(
            defaults.keepalive_interval(),
            defaults.keepalive_timeout(),
            Duration::MAX,
            Duration::from_secs(1),
            defaults.idle_timeout(),
            defaults.first_request_timeout(),
        )
        .expect_err("overflow rejected")
        .reason(),
        GrpcTransportTimeoutsErrorReason::AgeOverflow
    );
}
