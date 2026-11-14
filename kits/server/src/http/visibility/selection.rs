// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Route selection shared by visibility checks and per-route limits.

/// How a route visibility rule matches a request target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpRouteMatchKind {
    /// Match the exact request path or RPC method path.
    Exact,
    /// Match this path and any child path below it.
    Prefix,
}

pub(super) fn select_route_rule<'route, Rule>(
    rules: impl IntoIterator<Item = (Rule, &'route str, HttpRouteMatchKind)>,
    path: &str,
) -> Option<Rule> {
    select_matching_rule(rules.into_iter().filter_map(|(rule, route, kind)| {
        matches_route(path, route, kind).then_some((rule, route.len(), kind))
    }))
}

pub(super) fn select_matching_rule<Rule>(
    matches: impl IntoIterator<Item = (Rule, usize, HttpRouteMatchKind)>,
) -> Option<Rule> {
    let mut winner = None;
    // Exact wins at equal length. Rank all matches before checking
    // listener permissions so a denied winner cannot fall through.
    for (rule, length, kind) in matches {
        let exact = kind == HttpRouteMatchKind::Exact;
        if winner
            .as_ref()
            .is_none_or(|(_, (best_length, best_exact))| {
                length > *best_length || (length == *best_length && (exact || !best_exact))
            })
        {
            winner = Some((rule, (length, exact)));
        }
    }
    winner.map(|(rule, _)| rule)
}

fn matches_route(path: &str, route: &str, kind: HttpRouteMatchKind) -> bool {
    if path == route {
        return true;
    }
    if kind == HttpRouteMatchKind::Exact {
        return false;
    }
    // A prefix owns its descendants at segment boundaries. The root prefix
    // serves as a catch-all for policies that explicitly configure one.
    route == "/"
        || path
            .as_bytes()
            .strip_prefix(route.as_bytes())
            .is_some_and(|suffix| suffix.first() == Some(&b'/'))
}
