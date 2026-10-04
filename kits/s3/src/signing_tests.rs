// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::SecretString;
use time::macros::datetime;

use crate::{EMPTY_SHA256_HEX, S3ObjectKey, S3SignedMethod, S3StorageConfig, sign_object_request};

fn config() -> S3StorageConfig {
    S3StorageConfig::new(
        String::from("https://objects.example.com"),
        String::from("eu-central-1"),
        String::from("archive"),
        SecretString::new(String::from("access-key").into_boxed_str()),
        SecretString::new(String::from("secret-key").into_boxed_str()),
        String::from("objects/v1"),
    )
    .expect("valid signing fixture")
}

#[test]
fn put_if_absent_signature_commits_to_conditional_header() {
    let config = config();
    let key = S3ObjectKey::new(String::from("objects/v1/chunks/01.pb"))
        .expect("valid signing key fixture");
    let signed = sign_object_request(
        &config,
        S3SignedMethod::PutIfAbsent,
        &key,
        b"ciphertext",
        datetime!(2026-05-14 10:11:12 UTC),
    )
    .expect("request signing should succeed");
    assert_eq!(
        signed.object_url().as_str(),
        "https://objects.example.com/archive/objects/v1/chunks/01.pb"
    );
    assert!(
        signed
            .authorization()
            .contains("SignedHeaders=host;if-none-match;x-amz-content-sha256;x-amz-date")
    );
    assert!(!signed.authorization().contains("secret-key"));
    assert!(!format!("{signed:?}").contains("access-key"));
}

#[test]
fn get_signature_uses_empty_payload_hash() {
    let config = config();
    let key = S3ObjectKey::new(String::from("objects/v1/chunks/01.pb"))
        .expect("valid signing key fixture");
    let signed = sign_object_request(
        &config,
        S3SignedMethod::Get,
        &key,
        &[],
        datetime!(2026-05-14 10:11:12 UTC),
    )
    .expect("request signing should succeed");
    assert_eq!(signed.payload_hash(), EMPTY_SHA256_HEX);
    assert!(!signed.authorization().contains("if-none-match"));
}

#[test]
fn delete_signature_has_only_read_delete_headers() {
    let config = config();
    let key = S3ObjectKey::new(String::from("objects/v1/chunks/01.pb"))
        .expect("valid signing key fixture");
    let signed = sign_object_request(
        &config,
        S3SignedMethod::Delete,
        &key,
        &[],
        datetime!(2026-05-14 10:11:12 UTC),
    )
    .expect("request signing should succeed");
    assert_eq!(signed.method(), S3SignedMethod::Delete);
    assert_eq!(signed.payload_hash(), EMPTY_SHA256_HEX);
    assert!(
        signed
            .authorization()
            .contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date")
    );
    assert!(!signed.authorization().contains("if-none-match"));
}

#[test]
fn body_is_rejected_for_non_upload_operations() {
    let config = config();
    let key = S3ObjectKey::new(String::from("objects/v1/chunks/01.pb"))
        .expect("valid signing key fixture");
    for method in [
        S3SignedMethod::Get,
        S3SignedMethod::Head,
        S3SignedMethod::Delete,
    ] {
        assert!(
            sign_object_request(
                &config,
                method,
                &key,
                b"unexpected",
                datetime!(2026-05-14 10:11:12 UTC),
            )
            .is_err()
        );
    }
}
