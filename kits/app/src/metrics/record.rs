// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use metrics::{Counter, counter};

use super::{AppMetricName, AppMetricNamespace};

const APP_EVENT_COUNTER: &str = "reallyme_app_events_total";
const APP_METRIC_SERIES_REJECTED_COUNTER: &str = "reallyme_app_metric_series_rejected_total";
const APP_METRIC_NAMESPACE_LABEL: &str = "app_namespace";
const APP_METRIC_NAME_LABEL: &str = "app_metric";

// The process-wide cap bounds both our retained handles and the exporter's
// registered series, even if callers generate many individually valid names.
const MAX_APP_METRIC_SERIES: usize = 1_024;

#[derive(Default)]
struct AppMetricCounterCache {
    counters: HashMap<String, HashMap<String, Counter>>,
    series_count: usize,
    rejected_counter: Option<Counter>,
}

fn metric_cache() -> &'static Mutex<AppMetricCounterCache> {
    static CACHE: OnceLock<Mutex<AppMetricCounterCache>> = OnceLock::new();

    CACHE.get_or_init(|| Mutex::new(AppMetricCounterCache::default()))
}

fn rejected_counter(cache: &mut AppMetricCounterCache) -> Counter {
    cache
        .rejected_counter
        .get_or_insert_with(|| counter!(APP_METRIC_SERIES_REJECTED_COUNTER))
        .clone()
}

fn metric_counter(
    cache: &Mutex<AppMetricCounterCache>,
    namespace: &AppMetricNamespace,
    name: &AppMetricName,
) -> Counter {
    let namespace = namespace.as_str();
    let name = name.as_str();

    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(error) => error.into_inner(),
    };

    if let Some(counter) = cache
        .counters
        .get(namespace)
        .and_then(|names| names.get(name))
    {
        return counter.clone();
    }

    let Some(next_series_count) = cache.series_count.checked_add(1) else {
        return rejected_counter(&mut cache);
    };
    if next_series_count > MAX_APP_METRIC_SERIES {
        return rejected_counter(&mut cache);
    }

    let counter = counter!(
        APP_EVENT_COUNTER,
        APP_METRIC_NAMESPACE_LABEL => namespace.to_owned(),
        APP_METRIC_NAME_LABEL => name.to_owned(),
    );
    cache
        .counters
        .entry(namespace.to_owned())
        .or_default()
        .insert(name.to_owned(), counter.clone());
    cache.series_count = next_series_count;

    counter
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
