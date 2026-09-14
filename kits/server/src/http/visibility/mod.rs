// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP listener identity and route visibility policy primitives.

mod error;
mod name;
mod route;
mod validation;

#[cfg(test)]
mod tests;

pub use error::{HttpRoutePolicyError, HttpRoutePolicyErrorReason};
pub use name::{
    HttpAuthPolicyName, HttpListenerIdentity, HttpListenerName, HttpListenerVisibility,
    HttpRateLimitTierName, HttpVisibilityClass,
};
pub use route::{
    HttpRouteMatchKind, HttpRoutePrefix, HttpRouteVisibility, HttpRouteVisibilityPolicy,
    HttpRouteVisibilityRule,
};
