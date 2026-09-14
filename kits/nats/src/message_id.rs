// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic message-id helpers for JetStream dedupe.

use reallyme_crypto::sha2;

/// Returns a deterministic message id derived from a subject and full payload.
pub fn deterministic_message_id_for_payload(subject: &str, payload: &[u8]) -> String {
    deterministic_message_id_from_parts(subject, &[payload])
}

/// Returns a deterministic message id derived from a subject and ordered parts.
pub fn deterministic_message_id_from_parts(subject: &str, parts: &[&[u8]]) -> String {
    let mut message = Vec::new();
    append_length_prefixed_bytes(&mut message, subject.as_bytes());

    for part in parts {
        append_length_prefixed_bytes(&mut message, part);
    }

    lowercase_hex(sha2::digest(message.as_slice()).as_bytes())
}

/// Returns a deterministic message id derived from arbitrary ordered text parts.
pub fn deterministic_message_id(subject: &str, parts: &[&str]) -> String {
    let byte_parts: Vec<&[u8]> = parts.iter().map(|part| part.as_bytes()).collect();
    deterministic_message_id_from_parts(subject, byte_parts.as_slice())
}

fn append_length_prefixed_bytes(message: &mut Vec<u8>, value: &[u8]) {
    let length = u64::try_from(value.len()).unwrap_or(u64::MAX);

    message.extend_from_slice(&length.to_be_bytes());
    message.extend_from_slice(value);
}

fn lowercase_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for byte in bytes {
        let high = (byte >> 4) as usize;
        let low = (byte & 0x0F) as usize;
        output.push(HEX[high] as char);
        output.push(HEX[low] as char);
    }

    output
}

#[cfg(test)]
#[path = "message_id_tests.rs"]
mod tests;
