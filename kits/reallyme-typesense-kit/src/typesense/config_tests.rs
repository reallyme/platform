// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use secrecy::SecretString;

use super::{TypesenseConfig, TypesenseConfigError, TypesenseConfigErrorReason, TypesenseEndpoint};

#[test]
fn rejects_endpoint_without_http_scheme() {
    let endpoint = TypesenseEndpoint::parse("ftp://localhost:8108");

    assert!(matches!(
        endpoint,
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::UnsupportedEndpointScheme
        })
    ));
}

#[test]
fn rejects_invalid_endpoint_url() {
    let endpoint = TypesenseEndpoint::parse("https://example.com::8108");

    assert!(matches!(
        endpoint,
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::MalformedEndpoint
        })
    ));
}

#[test]
fn rejects_endpoint_with_embedded_credentials() {
    let endpoint = TypesenseEndpoint::parse("https://alice:secret@typesense.internal");

    assert!(matches!(
        endpoint,
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::EndpointContainsUserInfo
        })
    ));
}

#[test]
fn trims_trailing_slash_from_endpoint() -> Result<(), TypesenseConfigError> {
    let endpoint = TypesenseEndpoint::parse("https://typesense.internal/")?;

    assert_eq!(endpoint.as_str(), "https://typesense.internal");
    Ok(())
}

#[test]
fn rejects_empty_api_key() -> Result<(), TypesenseConfigError> {
    let endpoint = TypesenseEndpoint::parse("https://typesense.internal")?;

    let config = TypesenseConfig::new(
        endpoint,
        SecretString::new(String::new().into()),
        Duration::from_secs(1),
    );

    assert!(matches!(
        config,
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::EmptyApiKey
        })
    ));
    Ok(())
}

#[test]
fn rejects_zero_request_timeout() -> Result<(), TypesenseConfigError> {
    let endpoint = TypesenseEndpoint::parse("https://typesense.internal")?;

    let config = TypesenseConfig::new(
        endpoint,
        SecretString::new(String::from("test-key").into()),
        Duration::ZERO,
    );

    assert!(matches!(
        config,
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::ZeroRequestTimeout
        })
    ));
    Ok(())
}
