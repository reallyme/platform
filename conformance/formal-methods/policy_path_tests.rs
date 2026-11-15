// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exhaustive proof for the escape decoder used by the HTTP visibility guard.

#[allow(dead_code, reason = "the harness calls the byte decoder in isolation")]
#[path = "../../kits/server/src/http/layers/route_visibility/policy_path.rs"]
mod policy_path;

use std::convert::TryFrom;

use policy_path::decode_policy_escape;

#[kani::proof]
fn percent_escape_matches_the_byte_policy() {
    let high: u8 = kani::any();
    let low: u8 = kani::any();
    let expected = char::from(high)
        .to_digit(16)
        .zip(char::from(low).to_digit(16))
        .and_then(|(high, low)| u8::try_from(high * 16 + low).ok())
        .filter(|byte| !matches!(*byte, b'/' | b'\\' | b'%' | b'\0'));
    assert_eq!(decode_policy_escape(high, low), expected);
}
