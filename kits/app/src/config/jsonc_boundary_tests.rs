// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_APP_JSONC_BYTES, parse_jsonc_config, strip_jsonc_comments};
#[test]
fn block_comments_do_not_join_json_tokens() {
    for text in ["1/*comment*/2", "tr/*comment*/ue", "-/*comment*/1"] {
        assert!(parse_jsonc_config::<serde_json::Value>(text).is_err());
    }
    assert_eq!(
        parse_jsonc_config::<u32>("/*before*/12/*after*/").expect("number"),
        12
    );
}
#[test]
fn comment_stripper_itself_bounds_public_input() {
    assert!(strip_jsonc_comments(&" ".repeat(MAX_APP_JSONC_BYTES + 1)).is_err());
}
