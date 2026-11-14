// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Proof of the listener guard's decision after matching each path spelling.

#[path = "../../kits/server/src/http/visibility/admission.rs"]
mod admission;

use admission::{PathPolicy, RouteAdmission, resolve_route_admission};

#[kani::proof]
fn admits_only_two_unmatched_paths_or_one_agreed_permitted_rule() {
    // Rule identities model equality. The production caller supplies the
    // selected rules and evaluates their listener restrictions separately.
    let raw: Option<(u32, bool)> = kani::any();
    let canonical: Option<(u32, bool)> = kani::any();
    let allow_unmatched: bool = kani::any();

    let to_policy = |selection| match selection {
        None => PathPolicy::Unmatched,
        Some((rule, listener_allowed)) => PathPolicy::Matched {
            rule,
            listener_allowed,
        },
    };
    let decision = resolve_route_admission(to_policy(raw), to_policy(canonical), allow_unmatched);

    assert_eq!(
        decision == RouteAdmission::AllowedByDefault,
        raw.is_none() && canonical.is_none() && allow_unmatched,
    );
    let agreed_rule = raw
        .filter(|(_, allowed)| *allowed)
        .filter(|selection| Some(*selection) == canonical)
        .map(|(rule, _)| rule);
    let admitted_rule = match decision {
        RouteAdmission::AllowedByRule(rule) => Some(rule),
        RouteAdmission::AllowedByDefault | RouteAdmission::Blocked => None,
    };
    assert_eq!(admitted_rule, agreed_rule);
}
