// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{RequestBodyLimitBytes, RequestTimeout};
use std::time::Duration;

#[test]
fn typed_timeout_and_body_limit_validation_remain_in_config() {
    assert!(RequestTimeout::new(Duration::ZERO).is_err());
    assert!(RequestBodyLimitBytes::new(0).is_err());
}
