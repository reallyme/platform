// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::{AppHealthContribution, ready_app_health_contribution};

use super::ExampleAppContext;

/// Returns this app's own health contribution.
pub fn app_health(_context: &ExampleAppContext) -> AppHealthContribution {
    ready_app_health_contribution()
}
