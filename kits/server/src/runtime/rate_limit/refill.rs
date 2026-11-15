// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::convert::TryFrom;
use std::num::NonZeroU32;

const NANOS_PER_SECOND: u128 = 1_000_000_000;

pub(super) struct BucketCredits {
    pub(super) tokens: u32,
    pub(super) fractional_tokens: u64,
}

pub(super) fn refill_credits(
    credits: BucketCredits,
    elapsed_nanos: u128,
    tokens_per_second: u32,
    divisor: NonZeroU32,
    capacity: u32,
    multiplier: u32,
) -> BucketCredits {
    let denominator = NANOS_PER_SECOND.saturating_mul(u128::from(divisor.get()));
    let accrued = elapsed_nanos
        .saturating_mul(u128::from(tokens_per_second))
        .saturating_mul(u128::from(multiplier))
        .saturating_add(u128::from(credits.fractional_tokens));
    let refill = u32::try_from(accrued / denominator).unwrap_or(u32::MAX);
    let tokens = credits.tokens.saturating_add(refill).min(capacity);
    // A full bucket cannot retain a fraction for use after its next debit.
    let fractional_tokens = if tokens == capacity {
        0
    } else {
        u64::try_from(accrued % denominator).unwrap_or_default()
    };
    BucketCredits {
        tokens,
        fractional_tokens,
    }
}
