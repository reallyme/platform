// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One canonical path spelling for visibility, body limits, and rate tiers.

use std::borrow::Cow;

pub(super) fn canonical_policy_path(path: &str) -> Option<Cow<'_, str>> {
    if !path.starts_with('/') || path.contains("//") || path.contains('\\') {
        return None;
    }

    let canonical = if path.as_bytes().contains(&b'%') {
        let source = path.as_bytes();
        let mut decoded = Vec::with_capacity(source.len());
        let mut offset = 0;
        while offset < source.len() {
            if source[offset] == b'%' {
                let end = offset.checked_add(3)?;
                let [high, low] = source.get(offset.checked_add(1)?..end)? else {
                    return None;
                };
                decoded.push(decode_policy_escape(*high, *low)?);
                offset = end;
            } else {
                decoded.push(source[offset]);
                offset = offset.checked_add(1)?;
            }
        }
        Cow::Owned(String::from_utf8(decoded).ok()?)
    } else {
        Cow::Borrowed(path)
    };

    if canonical.contains("//")
        || canonical
            .split('/')
            .any(|segment| matches!(segment, "." | ".."))
    {
        return None;
    }
    Some(canonical)
}

pub(super) fn decode_policy_escape(high: u8, low: u8) -> Option<u8> {
    let byte = hex_value(high)?
        .checked_mul(16)?
        .checked_add(hex_value(low)?)?;
    // A second decode, separator normalization, or NUL handling must never
    // change which route policy was selected.
    (!matches!(byte, b'/' | b'\\' | b'%' | b'\0')).then_some(byte)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
