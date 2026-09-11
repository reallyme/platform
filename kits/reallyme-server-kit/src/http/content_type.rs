// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::HeaderValue;

/// Prometheus text exposition content type.
pub const PROMETHEUS_TEXT_CONTENT_TYPE: HeaderValue =
    HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8");

const APPLICATION_JSON: &[u8] = b"application/json";
const JSON_STRUCTURED_SUFFIX: &[u8] = b"+json";

/// Returns whether a content type is JSON or a structured `+json` subtype.
///
/// HTTP media types are case-insensitive and may include parameters. This
/// helper avoids allocating in transport error paths while still preserving
/// explicit application JSON responses from service code.
pub(crate) fn header_value_is_json_content_type(value: &HeaderValue) -> bool {
    let media_type = media_type_without_parameters(value.as_bytes());
    let media_type = trim_ascii_whitespace(media_type);

    if media_type.eq_ignore_ascii_case(APPLICATION_JSON) {
        return true;
    }

    let Some(slash_index) = media_type.iter().position(|byte| *byte == b'/') else {
        return false;
    };
    let subtype = &media_type[slash_index + 1..];

    subtype.len() > JSON_STRUCTURED_SUFFIX.len()
        && subtype[subtype.len() - JSON_STRUCTURED_SUFFIX.len()..]
            .eq_ignore_ascii_case(JSON_STRUCTURED_SUFFIX)
}

fn media_type_without_parameters(value: &[u8]) -> &[u8] {
    match value.iter().position(|byte| *byte == b';') {
        Some(parameter_start) => &value[..parameter_start],
        None => value,
    }
}

fn trim_ascii_whitespace(value: &[u8]) -> &[u8] {
    let start = value
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|byte| !byte.is_ascii_whitespace())
        .map(|index| index + 1)
        .unwrap_or(start);

    &value[start..end]
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::header_value_is_json_content_type;

    #[test]
    fn json_content_type_detection_is_case_insensitive_and_parameter_safe() {
        assert!(header_value_is_json_content_type(
            &HeaderValue::from_static("application/json")
        ));
        assert!(header_value_is_json_content_type(
            &HeaderValue::from_static("Application/Json; Charset=UTF-8")
        ));
        assert!(header_value_is_json_content_type(
            &HeaderValue::from_static("application/problem+json")
        ));
        assert!(!header_value_is_json_content_type(
            &HeaderValue::from_static("text/plain")
        ));
    }
}
