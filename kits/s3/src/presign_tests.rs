// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use secrecy::SecretString;
use time::macros::datetime;

use super::{MAX_S3_PRESIGN_TTL_SECONDS, percent_encode};
use crate::{S3ObjectKey, S3StorageConfig, S3StorageError, presign_get_object};

fn config() -> Result<S3StorageConfig, S3StorageError> {
    S3StorageConfig::new(
        String::from("https://example.r2.cloudflarestorage.com"),
        String::from("auto"),
        String::from("archive"),
        SecretString::from(String::from("access/key +id")),
        SecretString::from(String::from("secret-key")),
        String::from("tenant/v1"),
    )
}

#[test]
fn presigned_get_is_deterministic_scoped_and_redacted() -> Result<(), S3StorageError> {
    let config = config()?;
    let key = config.scoped_object_key("chunks/01/02/03.pb")?;
    let issued = presign_get_object(&config, &key, 30, datetime!(2026-09-10 12:34:56 UTC))?;
    let url = issued.expose_url();
    assert!(url.starts_with(
        "https://example.r2.cloudflarestorage.com/archive/tenant/v1/chunks/01/02/03.pb?"
    ));
    assert!(url.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
    assert!(
        url.contains("X-Amz-Credential=access%2Fkey%20%2Bid%2F20260910%2Fauto%2Fs3%2Faws4_request")
    );
    assert!(url.contains("X-Amz-Date=20260910T123456Z"));
    assert!(url.contains("X-Amz-Expires=30"));
    assert!(url.contains("X-Amz-SignedHeaders=host"));
    assert!(url.contains("X-Amz-Signature="));
    assert_eq!(
        url,
        concat!(
            "https://example.r2.cloudflarestorage.com/archive/tenant/v1/chunks/01/02/03.pb?",
            "X-Amz-Algorithm=AWS4-HMAC-SHA256&",
            "X-Amz-Credential=access%2Fkey%20%2Bid%2F20260910%2Fauto%2Fs3%2Faws4_request&",
            "X-Amz-Date=20260910T123456Z&X-Amz-Expires=30&X-Amz-SignedHeaders=host&",
            "X-Amz-Signature=cfc4b3029e81c158b0eb19dbaefd4b5e1eabd0870eec747bc254181c71fb6c9e"
        )
    );
    assert!(!format!("{issued:?}").contains("X-Amz-Signature"));
    assert!(!url.contains("secret-key"));
    Ok(())
}

#[test]
fn presigned_get_rejects_zero_and_provider_excessive_expiry() -> Result<(), S3StorageError> {
    let config = config()?;
    let key = S3ObjectKey::new(String::from("tenant/v1/chunks/01.pb"))?;
    for expiry in [0, MAX_S3_PRESIGN_TTL_SECONDS.saturating_add(1)] {
        assert!(
            presign_get_object(&config, &key, expiry, datetime!(2026-09-10 12:34:56 UTC),).is_err()
        );
    }
    Ok(())
}

#[test]
fn sigv4_query_percent_encoding_never_uses_form_encoding() {
    assert_eq!(
        percent_encode(b"a/b c+d~")
            .expect("small input should encode")
            .as_str(),
        "a%2Fb%20c%2Bd~"
    );
}

#[test]
#[allow(clippy::expect_used)]
fn presign_regression_vector_remains_unchanged() {
    // Preserve this canonical path and expected signature independently of the
    // other example fixtures: changing either would conceal a regression.
    let config = S3StorageConfig::new(
        "https://example.r2.cloudflarestorage.com".to_owned(),
        "auto".to_owned(),
        "archive".to_owned(),
        SecretString::from("access/key +id"),
        SecretString::from("secret-key"),
        "tenant/v1".to_owned(),
    )
    .expect("original fixture");
    let key = config.scoped_object_key("chunks/01/02/03.pb").expect("key");
    let issued = presign_get_object(&config, &key, 30, datetime!(2026-09-10 12:34:56 UTC))
        .expect("presigned URL");
    assert_eq!(
        issued.expose_url(),
        concat!(
            "https://example.r2.cloudflarestorage.com/archive/tenant/v1/chunks/01/02/03.pb?",
            "X-Amz-Algorithm=AWS4-HMAC-SHA256&",
            "X-Amz-Credential=access%2Fkey%20%2Bid%2F20260910%2Fauto%2Fs3%2Faws4_request&",
            "X-Amz-Date=20260910T123456Z&X-Amz-Expires=30&X-Amz-SignedHeaders=host&",
            "X-Amz-Signature=cfc4b3029e81c158b0eb19dbaefd4b5e1eabd0870eec747bc254181c71fb6c9e"
        )
    );
}
