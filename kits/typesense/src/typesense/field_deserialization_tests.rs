// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::SearchFieldName;
#[test]
fn deserialization_enforces_constructor_validation() {
    for raw in ["../other", "name/../../keys", "field:!=secret", "", "a\\b"] {
        let json = serde_json::to_string(raw).expect("JSON string");
        assert!(serde_json::from_str::<SearchFieldName>(&json).is_err());
    }
    let parsed: SearchFieldName =
        serde_json::from_str("\"safe_name-1\"").expect("valid identifier");
    assert_eq!(parsed.as_str(), "safe_name-1");
    assert_eq!(
        serde_json::to_string(&parsed).expect("serialization"),
        "\"safe_name-1\""
    );
}
