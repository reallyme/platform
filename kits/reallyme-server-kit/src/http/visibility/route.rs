// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::error::{HttpRoutePolicyError, HttpRoutePolicyErrorReason};
use super::name::{
    HttpAuthPolicyName, HttpListenerName, HttpListenerVisibility, HttpRateLimitTierName,
    HttpVisibilityClass,
};
use super::validation::MAX_HTTP_ROUTE_PREFIX_BYTES;
use crate::config::RequestBodyLimitBytes;

/// Route visibility policy used by listener-level guards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRouteVisibility {
    /// Route is reachable only through public listeners.
    PublicOnly,
    /// Route is reachable only through private listeners.
    PrivateOnly,
    /// Route is reachable only through internal listeners.
    InternalOnly,
    /// Route is reachable through public and private listeners, but not internal listeners.
    PublicAndPrivate,
    /// Route is reachable through every configured listener visibility.
    All,
    /// Route is reachable through listeners with this custom visibility class.
    Custom(HttpVisibilityClass),
}

impl HttpRouteVisibility {
    /// Returns whether this route may be served by a listener with `visibility`.
    pub fn allows_listener(&self, visibility: &HttpListenerVisibility) -> bool {
        match (self, visibility) {
            (Self::All, _) => true,
            (
                Self::PublicAndPrivate,
                HttpListenerVisibility::Public | HttpListenerVisibility::Private,
            ) => true,
            (Self::PublicOnly, HttpListenerVisibility::Public)
            | (Self::PrivateOnly, HttpListenerVisibility::Private)
            | (Self::InternalOnly, HttpListenerVisibility::Internal) => true,
            (Self::Custom(left), HttpListenerVisibility::Custom(right)) => left == right,
            _ => false,
        }
    }
}

/// Validated route prefix for a listener visibility rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRoutePrefix(String);

impl HttpRoutePrefix {
    /// Creates a validated route prefix.
    pub fn new(value: impl Into<String>) -> Result<Self, HttpRoutePolicyError> {
        let value = value.into();

        if value.is_empty() {
            return Err(HttpRoutePolicyError::RoutePrefix {
                reason: HttpRoutePolicyErrorReason::Empty,
            });
        }

        if value.len() > MAX_HTTP_ROUTE_PREFIX_BYTES {
            return Err(HttpRoutePolicyError::RoutePrefix {
                reason: HttpRoutePolicyErrorReason::TooLong,
            });
        }

        if !value.starts_with('/')
            || value.contains('?')
            || value.contains('#')
            || value.contains("://")
            || value.contains('{')
            || value.contains('}')
            || value.split('/').any(|segment| segment.starts_with(':'))
            || value.chars().any(char::is_whitespace)
        {
            return Err(HttpRoutePolicyError::RoutePrefix {
                reason: HttpRoutePolicyErrorReason::InvalidRouteShape,
            });
        }

        Ok(Self(value))
    }

    /// Returns the validated route prefix.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn matches_path(&self, path: &str) -> bool {
        if self.0 == "/" || path == self.0 {
            return true;
        }

        path.strip_prefix(self.0.as_str())
            .map(|suffix| suffix.starts_with('/'))
            .unwrap_or(false)
    }
}

/// How a route visibility rule matches a request target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRouteMatchKind {
    /// Match the exact request path or RPC method path.
    Exact,
    /// Match this path and any child path below it.
    Prefix,
}

/// One listener-visibility rule for a stable route prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRouteVisibilityRule {
    route: HttpRoutePrefix,
    match_kind: HttpRouteMatchKind,
    visibility: HttpRouteVisibility,
    allowed_listeners: Vec<HttpListenerName>,
    rate_limit_tier: Option<HttpRateLimitTierName>,
    request_body_limit: Option<RequestBodyLimitBytes>,
    auth_policy: Option<HttpAuthPolicyName>,
}

/// Default action when no route visibility rule matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRouteVisibilityDefaultAction {
    /// Unmatched routes are allowed.
    Allow,
    /// Unmatched routes are denied.
    Deny,
}

impl HttpRouteVisibilityDefaultAction {
    const fn is_allow(self) -> bool {
        matches!(self, Self::Allow)
    }
}

impl HttpRouteVisibilityRule {
    /// Creates a visibility rule for one route prefix.
    pub fn new(prefix: HttpRoutePrefix, visibility: HttpRouteVisibility) -> Self {
        Self {
            route: prefix,
            match_kind: HttpRouteMatchKind::Prefix,
            visibility,
            allowed_listeners: Vec::new(),
            rate_limit_tier: None,
            request_body_limit: None,
            auth_policy: None,
        }
    }

    /// Creates a visibility rule for one exact HTTP/Connect/gRPC/WebSocket route.
    pub fn exact(route: HttpRoutePrefix, visibility: HttpRouteVisibility) -> Self {
        Self {
            route,
            match_kind: HttpRouteMatchKind::Exact,
            visibility,
            allowed_listeners: Vec::new(),
            rate_limit_tier: None,
            request_body_limit: None,
            auth_policy: None,
        }
    }

    /// Restricts this route rule to explicit listener names in addition to visibility.
    pub fn with_allowed_listeners(mut self, allowed_listeners: Vec<HttpListenerName>) -> Self {
        self.allowed_listeners = allowed_listeners;
        self
    }

    /// Attaches an optional route-level rate-limit tier policy hook.
    pub fn with_rate_limit_tier(mut self, rate_limit_tier: Option<HttpRateLimitTierName>) -> Self {
        self.rate_limit_tier = rate_limit_tier;
        self
    }

    /// Attaches an optional route-level request body size limit.
    pub fn with_request_body_limit(
        mut self,
        request_body_limit: Option<RequestBodyLimitBytes>,
    ) -> Self {
        self.request_body_limit = request_body_limit;
        self
    }

    /// Attaches an optional route-level auth policy hook.
    pub fn with_auth_policy(mut self, auth_policy: Option<HttpAuthPolicyName>) -> Self {
        self.auth_policy = auth_policy;
        self
    }

    /// Returns the route prefix.
    pub const fn prefix(&self) -> &HttpRoutePrefix {
        &self.route
    }

    /// Returns how this rule matches request paths.
    pub const fn match_kind(&self) -> HttpRouteMatchKind {
        self.match_kind
    }

    /// Returns the allowed route visibility.
    pub const fn visibility(&self) -> &HttpRouteVisibility {
        &self.visibility
    }

    /// Returns explicit allowed listener names.
    pub fn allowed_listeners(&self) -> &[HttpListenerName] {
        self.allowed_listeners.as_slice()
    }

    /// Returns the route-level rate-limit tier policy hook.
    pub const fn rate_limit_tier(&self) -> Option<&HttpRateLimitTierName> {
        self.rate_limit_tier.as_ref()
    }

    /// Returns the route-level request body size limit.
    pub const fn request_body_limit(&self) -> Option<RequestBodyLimitBytes> {
        self.request_body_limit
    }

    /// Returns the route-level auth policy hook.
    pub const fn auth_policy(&self) -> Option<&HttpAuthPolicyName> {
        self.auth_policy.as_ref()
    }

    fn allows_listener(
        &self,
        listener_name: &HttpListenerName,
        listener_visibility: &HttpListenerVisibility,
    ) -> bool {
        let explicitly_allowed = self.allowed_listeners.is_empty()
            || self
                .allowed_listeners
                .iter()
                .any(|allowed| allowed == listener_name);

        explicitly_allowed && self.visibility.allows_listener(listener_visibility)
    }

    fn match_score(&self, path: &str) -> Option<(usize, u8)> {
        match self.match_kind {
            HttpRouteMatchKind::Exact if path == self.route.as_str() => {
                Some((self.route.as_str().len(), 1))
            }
            HttpRouteMatchKind::Exact => None,
            HttpRouteMatchKind::Prefix if self.route.matches_path(path) => {
                Some((self.route.as_str().len(), 0))
            }
            HttpRouteMatchKind::Prefix => None,
        }
    }
}

/// Immutable route visibility policy evaluated before app handlers run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRouteVisibilityPolicy {
    rules: Vec<HttpRouteVisibilityRule>,
    default_action: HttpRouteVisibilityDefaultAction,
}

impl Default for HttpRouteVisibilityPolicy {
    fn default() -> Self {
        Self::allow_all()
    }
}

impl HttpRouteVisibilityPolicy {
    /// Creates a policy after validating duplicate route prefixes.
    pub fn new(
        rules: Vec<HttpRouteVisibilityRule>,
        default_action: HttpRouteVisibilityDefaultAction,
    ) -> Result<Self, HttpRoutePolicyError> {
        for (left_index, left) in rules.iter().enumerate() {
            for right in rules.iter().skip(left_index + 1) {
                if left.route == right.route && left.match_kind == right.match_kind {
                    return Err(HttpRoutePolicyError::RoutePrefix {
                        reason: HttpRoutePolicyErrorReason::Duplicate,
                    });
                }
            }
        }

        Ok(Self {
            rules,
            default_action,
        })
    }

    /// Creates a policy that allows unmatched routes.
    pub fn allow_by_default(
        rules: Vec<HttpRouteVisibilityRule>,
    ) -> Result<Self, HttpRoutePolicyError> {
        Self::new(rules, HttpRouteVisibilityDefaultAction::Allow)
    }

    /// Creates a policy that denies unmatched routes.
    pub fn deny_by_default(
        rules: Vec<HttpRouteVisibilityRule>,
    ) -> Result<Self, HttpRoutePolicyError> {
        Self::new(rules, HttpRouteVisibilityDefaultAction::Deny)
    }

    /// Creates an allow-all default policy with no matching rules.
    pub fn allow_all() -> Self {
        Self {
            rules: Vec::new(),
            default_action: HttpRouteVisibilityDefaultAction::Allow,
        }
    }

    /// Creates a deny-all default policy with no matching rules.
    pub fn deny_all() -> Self {
        Self {
            rules: Vec::new(),
            default_action: HttpRouteVisibilityDefaultAction::Deny,
        }
    }

    /// Returns the matching visibility rule for a path, if configured.
    pub fn rule_for_path(&self, path: &str) -> Option<&HttpRouteVisibilityRule> {
        self.rules
            .iter()
            .filter_map(|rule| rule.match_score(path).map(|score| (score, rule)))
            .max_by_key(|(score, _rule)| *score)
            .map(|(_score, rule)| rule)
    }

    /// Returns whether the configured route policy allows this listener.
    pub fn allows_request(
        &self,
        listener_name: &HttpListenerName,
        listener_visibility: &HttpListenerVisibility,
        path: &str,
    ) -> bool {
        self.rule_for_path(path)
            .map(|rule| rule.allows_listener(listener_name, listener_visibility))
            .unwrap_or_else(|| self.default_action.is_allow())
    }

    /// Returns the matching route rule for a request on the provided listener,
    /// or `None` when no rule matches or the match is blocked for this listener.
    pub fn matching_rule_for_request(
        &self,
        listener_name: &HttpListenerName,
        listener_visibility: &HttpListenerVisibility,
        path: &str,
    ) -> Option<&HttpRouteVisibilityRule> {
        self.rule_for_path(path).and_then(|rule| {
            rule.allows_listener(listener_name, listener_visibility)
                .then_some(rule)
        })
    }
}
