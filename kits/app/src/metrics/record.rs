// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use metrics::{Counter, counter};

use super::{AppMetricName, AppMetricNamespace};

const APP_EVENT_COUNTER: &str = "reallyme_app_events_total";
const APP_METRIC_SERIES_REJECTED_COUNTER: &str = "reallyme_app_metric_series_rejected_total";
const APP_METRIC_NAMESPACE_LABEL: &str = "app_namespace";
const APP_METRIC_NAME_LABEL: &str = "app_metric";

// The process-wide cap bounds retained label pairs, even if callers generate
// many individually valid names. Idle exporter series can be recreated.
const MAX_APP_METRIC_SERIES: usize = 1_024;

#[derive(Default)]
struct AppMetricCounterCache {
    series: HashMap<String, HashSet<String>>,
    series_count: usize,
}

fn metric_cache() -> &'static Mutex<AppMetricCounterCache> {
    static CACHE: OnceLock<Mutex<AppMetricCounterCache>> = OnceLock::new();

    CACHE.get_or_init(|| Mutex::new(AppMetricCounterCache::default()))
}

fn metric_counter(
    cache: &Mutex<AppMetricCounterCache>,
    namespace: &AppMetricNamespace,
    name: &AppMetricName,
) -> Counter {
    let namespace = namespace.as_str();
    let name = name.as_str();

    let allowed = {
        let mut cache = match cache.lock() {
            Ok(cache) => cache,
            Err(error) => error.into_inner(),
        };

        if cache
            .series
            .get(namespace)
            .is_some_and(|names| names.contains(name))
        {
            true
        } else if let Some(next_series_count) = cache.series_count.checked_add(1)
            && next_series_count <= MAX_APP_METRIC_SERIES
        {
            cache
                .series
                .entry(namespace.to_owned())
                .or_default()
                .insert(name.to_owned());
            cache.series_count = next_series_count;
            true
        } else {
            false
        }
    };

    if allowed {
        // Request the handle on every record so a recorder that evicts an idle
        // series can recreate it when events resume.
        counter!(
            APP_EVENT_COUNTER,
            APP_METRIC_NAMESPACE_LABEL => namespace.to_owned(),
            APP_METRIC_NAME_LABEL => name.to_owned(),
        )
    } else {
        counter!(APP_METRIC_SERIES_REJECTED_COUNTER)
    }
}

/// Records one app-level aggregate event.
///
/// The metric instrument name is stable; app namespace and metric name are
/// validated tokens with a process-wide series limit. App code must not use this for
/// user-specific, request-specific, handle-specific, or otherwise unbounded
/// labels. Once the process-wide series budget is full, new pairs increment
/// an unlabelled rejection counter; already registered pairs remain usable.
pub fn record_app_metric_counter(namespace: &AppMetricNamespace, name: &AppMetricName) {
    metric_counter(metric_cache(), namespace, name).increment(1);
}

/// Records one app-level aggregate event from static metric tokens.
///
/// This helper keeps simple app metric declaration boilerplate out of app
/// templates while still validating namespace/name values before recording.
pub fn record_app_metric_counter_by_name(
    namespace: &'static str,
    name: &'static str,
) -> Result<(), crate::AppKitError> {
    let namespace = AppMetricNamespace::new(namespace)?;
    let name = AppMetricName::new(name)?;

    record_app_metric_counter(&namespace, &name);

    Ok(())
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
