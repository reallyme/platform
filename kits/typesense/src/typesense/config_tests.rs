// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
fn plaintext_endpoints_are_only_valid_on_loopback() {
    for endpoint in ["http://typesense.internal:8108", "http://192.0.2.8:8108"] {
        assert!(matches!(
            TypesenseEndpoint::parse(endpoint),
            Err(TypesenseConfigError::Invalid {
                reason: TypesenseConfigErrorReason::PlaintextEndpointNotLoopback,
            })
        ));
    }
    assert!(TypesenseEndpoint::parse("http://127.0.0.1:8108").is_ok());
    assert!(TypesenseEndpoint::parse("http://[::1]:8108").is_ok());
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

#[test]
fn rejects_unbounded_deadlines_retries_and_connection_timing() -> Result<(), TypesenseConfigError> {
    let endpoint = TypesenseEndpoint::parse("https://typesense.internal")?;
    let excessive_timeout = TypesenseConfig::new(
        endpoint.clone(),
        SecretString::new(String::from("test-key").into()),
        Duration::from_secs(61),
    );
    assert!(matches!(
        excessive_timeout,
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::InvalidRequestDeadline
        })
    ));

    let config = TypesenseConfig::new(
        endpoint,
        SecretString::new(String::from("test-key").into()),
        Duration::from_secs(1),
    )?;
    assert!(matches!(
        config.with_max_retries(u8::MAX).validate(),
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::InvalidRetryPolicy
        })
    ));
    let config = TypesenseConfig::new(
        TypesenseEndpoint::parse("https://typesense.internal")?,
        SecretString::new(String::from("test-key").into()),
        Duration::from_secs(1),
    )?;
    assert!(matches!(
        config
            .with_tcp_keepalive(Some(Duration::from_secs(601)))
            .validate(),
        Err(TypesenseConfigError::Invalid {
            reason: TypesenseConfigErrorReason::InvalidConnectionTiming
        })
    ));
    Ok(())
}
