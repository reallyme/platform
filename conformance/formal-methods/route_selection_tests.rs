// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Proof of the precedence applied to matching route rules.

#[allow(dead_code, reason = "the harness isolates ranking after path matching")]
#[path = "../../kits/server/src/http/visibility/selection.rs"]
mod selection;

use selection::{HttpRouteMatchKind, select_matching_rule};

#[kani::proof]
fn greatest_length_wins_and_exact_breaks_ties() {
    let left_length: usize = kani::any();
    let right_length: usize = kani::any();
    let left_exact: bool = kani::any();
    let right_exact: bool = kani::any();
    let left_kind = match_kind(left_exact);
    let right_kind = match_kind(right_exact);

    let selected = select_matching_rule([
        (false, left_length, left_kind),
        (true, right_length, right_kind),
    ]);
    let left_has_priority =
        left_length > right_length || (left_length == right_length && left_exact && !right_exact);
    assert_eq!(selected, Some(!left_has_priority));
}

#[kani::proof]
fn no_match_has_no_winner_and_one_match_always_wins() {
    let length: usize = kani::any();
    let kind = match_kind(kani::any());
    assert_eq!(select_matching_rule::<u8>([]), None);
    assert_eq!(select_matching_rule([(0, length, kind)]), Some(0));
}

fn match_kind(exact: bool) -> HttpRouteMatchKind {
    if exact {
        HttpRouteMatchKind::Exact
    } else {
        HttpRouteMatchKind::Prefix
    }
}
