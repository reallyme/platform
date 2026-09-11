// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app metric naming helpers.

mod metric_name;
mod namespace;
mod record;

pub use metric_name::AppMetricName;
pub use namespace::AppMetricNamespace;
pub use record::{record_app_metric_counter, record_app_metric_counter_by_name};
