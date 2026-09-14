// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::Deserialize;

use super::{AppConfigParseErrorReason, MAX_APP_JSONC_BYTES, parse_jsonc_config};

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct ConfigFixture {
    url: String,
    enabled: bool,
}

#[test]
fn parses_jsonc_without_touching_url_strings() {
    let config: ConfigFixture = parse_jsonc_config(
        r#"{
            // comment
            "url": "https://api.reallyme.net/path",
            "enabled": true
        }"#,
    )
    .expect("valid JSONC fixture should parse");

    assert_eq!(
        config,
        ConfigFixture {
            url: "https://api.reallyme.net/path".to_owned(),
            enabled: true,
        }
    );
}

#[test]
fn rejects_unclosed_block_comment() {
    let result = parse_jsonc_config::<ConfigFixture>("{ /* unclosed");

    assert!(matches!(
        result.map_err(|error| error.reason()),
        Err(AppConfigParseErrorReason::UnclosedComment)
    ));
}

#[test]
fn rejects_too_long_jsonc_input() {
    let oversized = "x".repeat(MAX_APP_JSONC_BYTES + 1);

    let result = parse_jsonc_config::<ConfigFixture>(oversized.as_str());

    assert!(matches!(
        result.map_err(|error| error.reason()),
        Err(AppConfigParseErrorReason::TooLong)
    ));
}
