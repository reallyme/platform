// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Per-client entropy seeding for bounded retry jitter.

use std::convert::TryFrom;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{DEFAULT_RETRY_ENTROPY_SEED, NEXT_RETRY_ENTROPY_SEED};

pub(super) fn initial_retry_entropy_seed() -> u64 {
    let counter_seed = NEXT_RETRY_ENTROPY_SEED.fetch_add(0xA076_1D64_78BD_642F, Ordering::Relaxed);
    let time_seed = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let nanos = duration.as_nanos();
            let lower = nanos & u128::from(u64::MAX);
            u64::try_from(lower).unwrap_or(DEFAULT_RETRY_ENTROPY_SEED)
        }
        Err(_) => DEFAULT_RETRY_ENTROPY_SEED,
    };
    let mixed = advance_retry_entropy(counter_seed ^ time_seed.rotate_left(17));
    if mixed == 0 {
        DEFAULT_RETRY_ENTROPY_SEED
    } else {
        mixed
    }
}

pub(super) fn advance_retry_entropy(mut value: u64) -> u64 {
    if value == 0 {
        value = DEFAULT_RETRY_ENTROPY_SEED;
    }
    value ^= value << 13;
    value ^= value >> 7;
    value ^= value << 17;
    if value == 0 {
        DEFAULT_RETRY_ENTROPY_SEED
    } else {
        value
    }
}
