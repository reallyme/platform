// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use metrics::{Counter, counter};

use super::{AppMetricName, AppMetricNamespace};

const APP_EVENT_COUNTER: &str = "reallyme_app_events_total";
const APP_METRIC_NAMESPACE_LABEL: &str = "app_namespace";
const APP_METRIC_NAME_LABEL: &str = "app_metric";

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct StaticMetricLabelPair {
    namespace: &'static str,
    name: &'static str,
}

#[derive(Default)]
struct AppMetricCounterCache {
    interned_labels: HashMap<String, &'static str>,
    counters: HashMap<StaticMetricLabelPair, Counter>,
}

fn metric_cache() -> &'static Mutex<AppMetricCounterCache> {
    static CACHE: OnceLock<Mutex<AppMetricCounterCache>> = OnceLock::new();

    CACHE.get_or_init(|| Mutex::new(AppMetricCounterCache::default()))
}

fn intern_label(cache: &mut AppMetricCounterCache, label: &str) -> &'static str {
    if let Some(label) = cache.interned_labels.get(label).copied() {
        return label;
    }

    let boxed = label.to_owned().into_boxed_str();
    let leaked: &'static str = Box::leak(boxed);
    cache.interned_labels.insert(leaked.to_owned(), leaked);

    leaked
}

fn metric_counter(namespace: &AppMetricNamespace, name: &AppMetricName) -> Counter {
    let namespace = namespace.as_str();
    let name = name.as_str();

    let cache = metric_cache();
    let mut cache = match cache.lock() {
        Ok(cache) => cache,
        Err(error) => error.into_inner(),
    };

    let namespace = intern_label(&mut cache, namespace);
    let name = intern_label(&mut cache, name);
    let key = StaticMetricLabelPair { namespace, name };

    if let Some(counter) = cache.counters.get(&key) {
        return counter.clone();
    }

    let counter = counter!(
        APP_EVENT_COUNTER,
        APP_METRIC_NAMESPACE_LABEL => namespace,
        APP_METRIC_NAME_LABEL => name,
    );
    cache.counters.insert(key, counter.clone());

    counter
}

/// Records one app-level aggregate event.
///
/// The metric instrument name is stable; app namespace and metric name are
/// validated low-cardinality labels. App code must not use this for
/// user-specific, request-specific, handle-specific, or otherwise unbounded
/// labels.
pub fn record_app_metric_counter(namespace: &AppMetricNamespace, name: &AppMetricName) {
    metric_counter(namespace, name).increment(1);
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
