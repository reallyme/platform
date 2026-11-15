// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/server/src/runtime/rate_limit/refill.rs"]
mod refill;

use std::num::NonZeroU32;

use refill::{BucketCredits, refill_credits};

#[kani::proof]
fn refill_keeps_credits_within_capacity() {
    let tokens: u32 = kani::any();
    let capacity: u32 = kani::any();
    let divisor: u32 = kani::any();
    let multiplier: u32 = kani::any();
    kani::assume(tokens <= capacity);
    kani::assume((1..=8).contains(&divisor));
    kani::assume((1..=2).contains(&multiplier));
    let divisor = NonZeroU32::new(divisor).unwrap();
    let result = refill_credits(
        BucketCredits {
            tokens,
            fractional_tokens: kani::any(),
        },
        kani::any(),
        kani::any(),
        divisor,
        capacity,
        multiplier,
    );
    assert!(result.tokens >= tokens);
    assert!(result.tokens <= capacity);
    if result.tokens == capacity {
        assert_eq!(result.fractional_tokens, 0);
    }
}

#[kani::proof]
fn subtoken_credit_is_carried_without_filling_the_bucket() {
    let fractional_tokens: u64 = kani::any();
    let elapsed_nanos: u32 = kani::any();
    kani::assume(fractional_tokens < 1_000_000_000);
    kani::assume(elapsed_nanos < 1_000_000_000);
    let result = refill_credits(
        BucketCredits {
            tokens: 0,
            fractional_tokens,
        },
        u128::from(elapsed_nanos),
        1,
        NonZeroU32::MIN,
        2,
        1,
    );
    let expected = fractional_tokens + u64::from(elapsed_nanos);
    assert_eq!(u64::from(result.tokens), expected / 1_000_000_000);
    assert_eq!(result.fractional_tokens, expected % 1_000_000_000);
}
