// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/server/src/grpc/deadline_wire.rs"]
mod wire;

use std::num::NonZeroU32;

#[kani::proof]
#[kani::unwind(12)]
fn wire_timeout_requires_the_protocol_grammar() {
    let bytes: [u8; 10] = kani::any();
    let len: usize = kani::any();
    kani::assume(len <= bytes.len());
    let input = &bytes[..len];
    let parsed = wire::parse_timeout_shape(input);
    let valid = (2..=9).contains(&len)
        && input.split_last().is_some_and(|(unit, digits)| {
            matches!(unit, b'H' | b'M' | b'S' | b'm' | b'u' | b'n')
                && digits.iter().all(u8::is_ascii_digit)
                && digits.iter().any(|digit| *digit != b'0')
        });
    assert_eq!(parsed.is_ok(), valid);
}

#[kani::proof]
fn appending_a_validated_digit_preserves_the_decimal_bound() {
    let seven_digit_prefix: u32 = kani::any();
    let digit: u8 = kani::any();
    kani::assume(seven_digit_prefix <= 9_999_999);
    kani::assume(digit.is_ascii_digit());
    let amount = wire::append_digit(seven_digit_prefix, &digit).expect("fits in eight digits");
    assert_eq!(amount, seven_digit_prefix * 10 + u32::from(digit - b'0'));
    assert!(amount <= 99_999_999);
}

#[kani::proof]
fn every_supported_unit_produces_a_positive_bounded_duration() {
    let count: u32 = kani::any();
    kani::assume((1..=99_999_999).contains(&count));
    let count = NonZeroU32::new(count).unwrap();
    let unit = match kani::any::<u8>() {
        0 => wire::TimeoutUnit::Hours,
        1 => wire::TimeoutUnit::Minutes,
        2 => wire::TimeoutUnit::Seconds,
        3 => wire::TimeoutUnit::Millis,
        4 => wire::TimeoutUnit::Micros,
        _ => wire::TimeoutUnit::Nanos,
    };
    let duration = unit.duration(count).expect("decoded timeout fits");
    assert!(!duration.is_zero());
    assert!(duration <= wire::MAX_GRPC_TIMEOUT);
}
