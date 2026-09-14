// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ValkeyCommandErrorReason, ValkeyRetryHint};

#[test]
fn retry_hints_never_claim_an_operation_is_safe_to_replay() {
    assert_eq!(
        ValkeyCommandErrorReason::ConnectionUnavailable.retry_hint(),
        ValkeyRetryHint::RetryIdempotentOperationAfterBackoff
    );
    assert_eq!(
        ValkeyCommandErrorReason::Timeout.retry_hint(),
        ValkeyRetryHint::RetryIdempotentOperation
    );
    assert_eq!(
        ValkeyCommandErrorReason::Rejected.retry_hint(),
        ValkeyRetryHint::DoNotRetry
    );
    assert_eq!(
        ValkeyCommandErrorReason::InvalidResponse.retry_hint(),
        ValkeyRetryHint::DoNotRetry
    );
}
