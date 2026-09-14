// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ApiKey, BearerToken, Credentials, ServiceToken};

#[test]
fn credentials_debug_redacts_secret_values() {
    let bearer = format!(
        "{:?}",
        Credentials::BearerToken(BearerToken::new("bearer-secret"))
    );
    let api_key = format!("{:?}", Credentials::ApiKey(ApiKey::new("api-key-secret")));
    let service = format!(
        "{:?}",
        Credentials::ServiceToken(ServiceToken::new("service-token-secret"))
    );

    assert!(!bearer.contains("bearer-secret"));
    assert!(!api_key.contains("api-key-secret"));
    assert!(!service.contains("service-token-secret"));
    assert!(bearer.contains("[REDACTED]"));
    assert!(api_key.contains("[REDACTED]"));
    assert!(service.contains("[REDACTED]"));
}

#[test]
fn credential_constant_time_eq_covers_secret_comparison_edges() {
    {
        let token = BearerToken::new("bearer-secret");
        assert!(token.constant_time_eq("bearer-secret"));
        assert!(!token.constant_time_eq("bearer-other"));
        assert!(!token.constant_time_eq("bearer-secret-extra"));
        assert!(BearerToken::new("").constant_time_eq(""));
    }

    {
        let key = ApiKey::new("api-key-secret");
        assert!(key.constant_time_eq("api-key-secret"));
        assert!(!key.constant_time_eq("api-key-other"));
        assert!(!key.constant_time_eq("api-key-secret-extra"));
        assert!(ApiKey::new("").constant_time_eq(""));
    }

    {
        let token = ServiceToken::new("service-token-secret");
        assert!(token.constant_time_eq("service-token-secret"));
        assert!(!token.constant_time_eq("service-token-other"));
        assert!(!token.constant_time_eq("service-token-secret-extra"));
        assert!(ServiceToken::new("").constant_time_eq(""));
    }
}
