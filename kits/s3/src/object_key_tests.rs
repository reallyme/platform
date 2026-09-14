// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::S3ObjectKey;

#[test]
fn object_key_accepts_expected_segments() {
    let result = S3ObjectKey::new(String::from("example/logs/node-1/report.json"));
    assert!(result.is_ok());
    let key = match result {
        Ok(value) => value,
        Err(_) => return,
    };
    assert_eq!(key.as_str(), "example/logs/node-1/report.json");
}

#[test]
fn object_key_rejects_parent_segments() {
    assert!(S3ObjectKey::new(String::from("../unsafe")).is_err());
}
