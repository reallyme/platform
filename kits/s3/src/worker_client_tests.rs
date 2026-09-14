// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::SecretString;

use super::{
    S3StorageConfig, S3StorageError, S3StorageErrorReason, WorkerS3StorageClient,
    validate_content_length,
};

fn config() -> Option<S3StorageConfig> {
    S3StorageConfig::new(
        String::from("https://objects.example.com"),
        String::from("eu-central-1"),
        String::from("archive"),
        SecretString::new(String::from("access-key").into_boxed_str()),
        SecretString::new(String::from("secret-key").into_boxed_str()),
        String::from("objects/v1"),
    )
    .ok()
}

#[test]
fn worker_client_debug_redacts_credentials() {
    let Some(config) = config() else {
        return;
    };
    let debug = format!("{:?}", WorkerS3StorageClient::new(config));
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("access-key"));
    assert!(!debug.contains("secret-key"));
}

#[test]
fn content_length_is_rejected_before_streaming() {
    assert!(matches!(
        validate_content_length(Some("101"), 100),
        Err(S3StorageError {
            reason: S3StorageErrorReason::ObjectTooLarge,
        })
    ));
}

#[test]
fn malformed_content_length_is_rejected() {
    assert!(matches!(
        validate_content_length(Some("not-a-number"), 100),
        Err(S3StorageError {
            reason: S3StorageErrorReason::DownloadUnavailable,
        })
    ));
}
