// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    S3ObjectKey, S3PutObjectRequest, S3SignedMethod, S3StorageConfig, presign_get_object,
    sign_object_request,
};
use secrecy::SecretString;
use time::macros::datetime;

fn config(endpoint: &str) -> S3StorageConfig {
    S3StorageConfig::new(
        endpoint.to_owned(),
        "eu-central-1".to_owned(),
        "archive".to_owned(),
        SecretString::from("access-key"),
        SecretString::from("secret-key"),
        "objects/v1".to_owned(),
    )
    .expect("valid fixture")
}

#[test]
fn canonical_authority_retains_nondefault_ports() {
    for (endpoint, authority) in [
        ("https://objects.example.com", "objects.example.com"),
        ("https://objects.example.com:443", "objects.example.com"),
        (
            "https://objects.example.com:9443",
            "objects.example.com:9443",
        ),
        ("https://[::1]:9443", "[::1]:9443"),
    ] {
        let url = url::Url::parse(endpoint).expect("URL");
        assert_eq!(
            super::canonical_host_header(&url).expect("authority"),
            authority
        );
    }
}

#[test]
fn signatures_use_utc_across_calendar_boundaries() {
    let config = config("https://objects.example.com:9443");
    let key = S3ObjectKey::new("item".to_owned()).expect("key");
    let utc = datetime!(2026-05-14 23:11:12 UTC);
    let offset = datetime!(2026-05-15 01:11:12 +02:00);
    let first = sign_object_request(&config, S3SignedMethod::Get, &key, &[], utc).expect("UTC");
    let second =
        sign_object_request(&config, S3SignedMethod::Get, &key, &[], offset).expect("offset");
    assert_eq!(first.amz_date(), "20260514T231112Z");
    assert_eq!(first.authorization(), second.authorization());
    // Independently computed with Python hashlib/hmac from the canonical request.
    assert!(
        first.authorization().ends_with(
            "Signature=3c3ea1bb063488c9c8d559586a7ad6b21a197e059ab65c7b66998f3f6eb3491d"
        )
    );
    assert_eq!(
        presign_get_object(&config, &key, 60, utc)
            .expect("UTC")
            .expose_url(),
        presign_get_object(&config, &key, 60, offset)
            .expect("offset")
            .expose_url()
    );
}

#[test]
fn bucket_and_region_cannot_change_the_signed_path_or_scope() {
    for bucket in ["..", ".", "a/b", "%2e%2e", "a\\b", "a?b"] {
        assert!(
            S3StorageConfig::new(
                "https://objects.example.com".to_owned(),
                "auto".to_owned(),
                bucket.to_owned(),
                SecretString::from("key"),
                SecretString::from("secret"),
                "prefix".to_owned()
            )
            .is_err()
        );
    }
    for region in ["a/b", "a\nb", "a b"] {
        assert!(
            S3StorageConfig::new(
                "https://objects.example.com".to_owned(),
                region.to_owned(),
                "bucket".to_owned(),
                SecretString::from("key"),
                SecretString::from("secret"),
                "prefix".to_owned()
            )
            .is_err()
        );
    }
}

#[test]
fn upload_debug_does_not_expose_payload() {
    let request = S3PutObjectRequest::new(
        S3ObjectKey::new("item".to_owned()).expect("key"),
        "application/octet-stream".to_owned(),
        b"private payload".to_vec(),
    )
    .expect("upload");
    let debug = format!("{request:?}");
    assert!(debug.contains("redacted"));
    assert!(!debug.contains("private"));
    assert!(!debug.contains("112, 114"));
}

#[test]
fn legacy_bucket_names_and_custom_signing_regions_remain_usable() {
    let bucket = "B".repeat(255);
    let config = S3StorageConfig::new(
        "https://objects.example.com".to_owned(),
        "custom_region-1".to_owned(),
        bucket.clone(),
        SecretString::from("key"),
        SecretString::from("secret"),
        "prefix".to_owned(),
    )
    .expect("legacy bucket");
    let key = config.scoped_object_key("item").expect("key");
    let signed = sign_object_request(
        &config,
        S3SignedMethod::Get,
        &key,
        &[],
        datetime!(2026-09-10 12:34:56 UTC),
    )
    .expect("signature");
    assert_eq!(signed.object_url().path(), format!("/{bucket}/prefix/item"));
    assert!(
        signed
            .authorization()
            .contains("/custom_region-1/s3/aws4_request")
    );
}
