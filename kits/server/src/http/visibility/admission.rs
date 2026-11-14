// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Admission decision for the raw and canonical spellings of a request path.

pub(super) enum PathPolicy<Rule> {
    Unmatched,
    Matched { rule: Rule, listener_allowed: bool },
}

#[must_use]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RouteAdmission<Rule> {
    AllowedByRule(Rule),
    AllowedByDefault,
    Blocked,
}

pub(super) fn resolve_route_admission<Rule: Eq>(
    raw: PathPolicy<Rule>,
    canonical: PathPolicy<Rule>,
    allow_unmatched: bool,
) -> RouteAdmission<Rule> {
    // A matched denial cannot inherit the unmatched default. Two explicit
    // matches must agree because their rules also supply per-route limits.
    match (raw, canonical) {
        (PathPolicy::Unmatched, PathPolicy::Unmatched) if allow_unmatched => {
            RouteAdmission::AllowedByDefault
        }
        (
            PathPolicy::Matched {
                rule: raw_rule,
                listener_allowed: true,
            },
            PathPolicy::Matched {
                rule: canonical_rule,
                listener_allowed: true,
            },
        ) if raw_rule == canonical_rule => RouteAdmission::AllowedByRule(raw_rule),
        _ => RouteAdmission::Blocked,
    }
}
