// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    HttpListenerName, HttpListenerVisibility, HttpRoutePrefix, HttpRouteVisibility,
    HttpRouteVisibilityPolicy, HttpRouteVisibilityRule,
};

#[test]
fn route_visibility_matches_expected_listener_types() {
    assert!(HttpRouteVisibility::PublicAndPrivate.allows_listener(&HttpListenerVisibility::Public));
    assert!(
        HttpRouteVisibility::PublicAndPrivate.allows_listener(&HttpListenerVisibility::Private)
    );
    assert!(
        !HttpRouteVisibility::PublicAndPrivate.allows_listener(&HttpListenerVisibility::Internal)
    );
    assert!(HttpRouteVisibility::All.allows_listener(&HttpListenerVisibility::Public));
    assert!(HttpRouteVisibility::All.allows_listener(&HttpListenerVisibility::Private));
    assert!(HttpRouteVisibility::All.allows_listener(&HttpListenerVisibility::Internal));
    assert!(HttpRouteVisibility::PublicOnly.allows_listener(&HttpListenerVisibility::Public));
    assert!(!HttpRouteVisibility::PublicOnly.allows_listener(&HttpListenerVisibility::Private));
    assert!(HttpRouteVisibility::PrivateOnly.allows_listener(&HttpListenerVisibility::Private));
    assert!(!HttpRouteVisibility::PrivateOnly.allows_listener(&HttpListenerVisibility::Public));
}

#[test]
fn exact_rules_override_less_specific_prefix_rules() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![
        HttpRouteVisibilityRule::new(
            HttpRoutePrefix::new("/internal").expect("valid route prefix"),
            HttpRouteVisibility::PrivateOnly,
        ),
        HttpRouteVisibilityRule::exact(
            HttpRoutePrefix::new("/internal/public-status").expect("valid exact route"),
            HttpRouteVisibility::PublicOnly,
        ),
    ])
    .expect("unique route rules");

    assert_eq!(
        policy
            .rule_for_path("/internal/public-status")
            .map(HttpRouteVisibilityRule::visibility),
        Some(&HttpRouteVisibility::PublicOnly)
    );
    assert_eq!(
        policy
            .rule_for_path("/internal/private-status")
            .map(HttpRouteVisibilityRule::visibility),
        Some(&HttpRouteVisibility::PrivateOnly)
    );
}

#[test]
fn route_visibility_policy_uses_validated_prefixes() {
    let policy = HttpRouteVisibilityPolicy::allow_by_default(vec![HttpRouteVisibilityRule::new(
        HttpRoutePrefix::new("/internal").expect("valid route prefix"),
        HttpRouteVisibility::PrivateOnly,
    )])
    .expect("unique route prefix");

    assert_eq!(
        policy
            .rule_for_path("/internal/metrics")
            .map(HttpRouteVisibilityRule::visibility),
        Some(&HttpRouteVisibility::PrivateOnly)
    );
    assert_eq!(policy.rule_for_path("/internalx"), None);
    assert_eq!(policy.rule_for_path("/public"), None);
}

#[test]
fn validates_listener_names_and_route_prefixes() {
    assert!(HttpListenerName::new("public-ingress").is_ok());
    assert!(HttpListenerName::new("Public").is_err());
    assert!(HttpRoutePrefix::new("/api/v1").is_ok());
    assert!(HttpRoutePrefix::new("https://api.example.com").is_err());
    assert!(HttpRoutePrefix::new("/api?x=1").is_err());
    assert!(HttpRoutePrefix::new("/users/:id").is_err());
    assert!(HttpRoutePrefix::new("/users/{id}").is_err());
}

#[test]
fn policy_default_action_defaults_to_allow_or_deny_as_configured() {
    let allow_by_default =
        HttpRouteVisibilityPolicy::allow_by_default(vec![]).expect("empty policy should be valid");
    let deny_by_default =
        HttpRouteVisibilityPolicy::deny_by_default(vec![]).expect("empty policy should be valid");

    assert!(
        allow_by_default.allows_request(
            &HttpListenerName::new("public").expect("valid listener name"),
            &HttpListenerVisibility::Public,
            "/anything",
        ),
        "allow-by-default policy should permit unmatched route"
    );
    assert!(
        !deny_by_default.allows_request(
            &HttpListenerName::new("public").expect("valid listener name"),
            &HttpListenerVisibility::Public,
            "/anything",
        ),
        "deny-by-default policy should block unmatched route"
    );
}
