// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    deterministic_message_id, deterministic_message_id_for_payload,
    deterministic_message_id_from_parts,
};

#[test]
fn deterministic_payload_helper_is_stable() {
    let first = deterministic_message_id_for_payload("reallyme.spider.requests.v1", b"abc");
    let second = deterministic_message_id_for_payload("reallyme.spider.requests.v1", b"abc");

    assert_eq!(first, second);
}

#[test]
fn different_subjects_produce_different_ids() {
    let first = deterministic_message_id_for_payload("reallyme.spider.requests.v1", b"abc");
    let second = deterministic_message_id_for_payload("reallyme.spider.results.v1", b"abc");

    assert_ne!(first, second);
}

#[test]
fn different_parts_produce_different_ids() {
    let first = deterministic_message_id_from_parts("reallyme.profile.updated.v1", &[b"a", b"bc"]);
    let second = deterministic_message_id_from_parts("reallyme.profile.updated.v1", &[b"ab", b"c"]);

    assert_ne!(first, second);
}

#[test]
fn string_parts_helper_is_deterministic() {
    let first = deterministic_message_id("reallyme.handle.assigned.v1", &["alice", "42"]);
    let second = deterministic_message_id("reallyme.handle.assigned.v1", &["alice", "42"]);

    assert_eq!(first, second);
}
