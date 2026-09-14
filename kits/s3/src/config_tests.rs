// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::SecretString;

use super::S3StorageConfig;

#[test]
fn config_debug_redacts_credentials() {
    let result = S3StorageConfig::new(
        String::from("https://objects.example.com"),
        String::from("eu-central-1"),
        String::from("logs"),
        SecretString::new(String::from("access-key").into_boxed_str()),
        SecretString::new(String::from("secret-key").into_boxed_str()),
        String::from("example/logs"),
    );
    assert!(result.is_ok());
    let config = match result {
        Ok(value) => value,
        Err(_) => return,
    };

    let debug = format!("{config:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-key"));
    assert!(!debug.contains("access-key"));
}

#[test]
fn config_rejects_missing_required_values() {
    assert!(
        S3StorageConfig::new(
            String::new(),
            String::from("eu-central-1"),
            String::from("logs"),
            SecretString::new(String::from("key").into_boxed_str()),
            SecretString::new(String::from("secret").into_boxed_str()),
            String::from("example/logs"),
        )
        .is_err()
    );
}

#[test]
fn config_accepts_endpoint_configured_s3_provider() {
    let result = S3StorageConfig::new(
        String::from("https://objects.example.com:9443"),
        String::from("eu-central-1"),
        String::from("archive"),
        SecretString::new(String::from("key").into_boxed_str()),
        SecretString::new(String::from("secret").into_boxed_str()),
        String::from("objects/v1"),
    );
    assert!(result.is_ok());
}

#[test]
fn config_rejects_unsafe_or_ambiguous_endpoints() {
    let credentials = || {
        (
            SecretString::new(String::from("key").into_boxed_str()),
            SecretString::new(String::from("secret").into_boxed_str()),
        )
    };
    let (access_key, secret_key) = credentials();
    assert!(
        S3StorageConfig::new(
            String::from("http://objects.example.com"),
            String::from("eu-central-1"),
            String::from("archive"),
            access_key,
            secret_key,
            String::from("objects/v1"),
        )
        .is_err()
    );

    let (access_key, secret_key) = credentials();
    assert!(
        S3StorageConfig::new(
            String::from("https://user@objects.example.com"),
            String::from("eu-central-1"),
            String::from("archive"),
            access_key,
            secret_key,
            String::from("objects/v1"),
        )
        .is_err()
    );

    for endpoint in [
        "https://objects.example.com/prefix",
        "https://objects.example.com?bucket=archive",
        "https://objects.example.com#fragment",
    ] {
        let (access_key, secret_key) = credentials();
        assert!(
            S3StorageConfig::new(
                String::from(endpoint),
                String::from("eu-central-1"),
                String::from("archive"),
                access_key,
                secret_key,
                String::from("objects/v1"),
            )
            .is_err()
        );
    }
}
