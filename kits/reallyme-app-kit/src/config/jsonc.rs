// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde::de::DeserializeOwned;
use thiserror::Error;

/// JSONC app config parse error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("app JSONC config failed parsing")]
pub struct AppConfigParseError {
    reason: AppConfigParseErrorReason,
}

impl AppConfigParseError {
    /// Constructs a parse error.
    pub const fn new(reason: AppConfigParseErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the typed parse failure reason.
    pub const fn reason(self) -> AppConfigParseErrorReason {
        self.reason
    }
}

/// Low-cardinality JSONC parse failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppConfigParseErrorReason {
    /// A block comment was opened but never closed.
    UnclosedComment,
    /// The JSONC input exceeded the parse size limit.
    TooLong,
    /// The comment-stripped input was not valid JSON for the target type.
    InvalidJson,
}

/// Maximum accepted JSONC input size in bytes.
const MAX_APP_JSONC_BYTES: usize = 1_048_576;

/// Parses a host-provided JSONC app config string into a typed config document.
///
/// This helper never reads files and never includes raw config text in errors.
/// Hosts are responsible for obtaining the string from a file, env/secret
/// manager, or platform config object.
pub fn parse_jsonc_config<T>(value: &str) -> Result<T, AppConfigParseError>
where
    T: DeserializeOwned,
{
    if value.len() > MAX_APP_JSONC_BYTES {
        return Err(AppConfigParseError::new(AppConfigParseErrorReason::TooLong));
    }

    let stripped = strip_jsonc_comments(value)?;
    serde_json::from_str(stripped.as_str())
        .map_err(|_| AppConfigParseError::new(AppConfigParseErrorReason::InvalidJson))
}

/// Removes JSONC comments from caller-supplied text.
pub fn strip_jsonc_comments(value: &str) -> Result<String, AppConfigParseError> {
    if value.len() > MAX_APP_JSONC_BYTES {
        return Err(AppConfigParseError::new(AppConfigParseErrorReason::TooLong));
    }
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(character) = chars.next() {
        if in_string {
            output.push(character);

            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }

            continue;
        }

        match character {
            '"' => {
                in_string = true;
                output.push(character);
            }
            '/' if chars.peek() == Some(&'/') => {
                let _ = chars.next();
                for next in chars.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                // Comments are whitespace, not token concatenation: 1/*x*/2
                // must remain invalid rather than silently becoming 12.
                output.push(' ');
                let _ = chars.next();
                let mut closed = false;
                let mut previous = '\0';
                for next in chars.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                    }

                    if previous == '*' && next == '/' {
                        closed = true;
                        break;
                    }

                    previous = next;
                }

                if !closed {
                    return Err(AppConfigParseError::new(
                        AppConfigParseErrorReason::UnclosedComment,
                    ));
                }
            }
            _ => output.push(character),
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
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
}

#[cfg(test)]
mod boundary_tests {
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
}
