// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{HttpTransportTimeoutErrorReason, HttpTransportTimeouts};

#[test]
fn rejects_zero_and_unbounded_deadlines() {
    let valid = Duration::from_secs(1);
    assert_eq!(
        HttpTransportTimeouts::new(valid, valid, Duration::ZERO, valid)
            .map_err(|error| error.reason()),
        Err(HttpTransportTimeoutErrorReason::ZeroDuration)
    );
    assert_eq!(
        HttpTransportTimeouts::new(Duration::from_secs(86_401), valid, valid, valid)
            .map_err(|error| error.reason()),
        Err(HttpTransportTimeoutErrorReason::AboveMaximum)
    );
    assert!(HttpTransportTimeouts::new(valid, valid, valid, valid).is_ok());
}
