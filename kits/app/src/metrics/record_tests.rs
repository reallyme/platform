// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use metrics::{
    Counter, Gauge, Histogram, Key, KeyName, Metadata, Recorder, SharedString, Unit,
    with_local_recorder,
};

use super::{
    APP_EVENT_COUNTER, APP_METRIC_NAME_LABEL, APP_METRIC_NAMESPACE_LABEL,
    APP_METRIC_SERIES_REJECTED_COUNTER, AppMetricCounterCache, MAX_APP_METRIC_SERIES,
    metric_counter, record_app_metric_counter_by_name,
};
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};
use crate::metrics::{AppMetricName, AppMetricNamespace};

struct RegisteredCounter {
    name: String,
    labels: Vec<(String, String)>,
    value: Arc<AtomicU64>,
}

#[derive(Default)]
struct RecordingRecorder {
    counters: Mutex<Vec<RegisteredCounter>>,
}

impl RecordingRecorder {
    fn registrations(&self, name: &str) -> usize {
        self.counters
            .lock()
            .expect("test recorder lock")
            .iter()
            .filter(|counter| counter.name == name)
            .count()
    }

    fn value(&self, name: &str, labels: &[(String, String)]) -> Option<u64> {
        self.counters
            .lock()
            .expect("test recorder lock")
            .iter()
            .find(|counter| counter.name == name && counter.labels == labels)
            .map(|counter| counter.value.load(Ordering::Relaxed))
    }

    fn evict(&self, name: &str, labels: &[(String, String)]) {
        self.counters
            .lock()
            .expect("test recorder lock")
            .retain(|counter| counter.name != name || counter.labels != labels);
    }
}

impl Recorder for RecordingRecorder {
    fn describe_counter(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_gauge(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn describe_histogram(&self, _: KeyName, _: Option<Unit>, _: SharedString) {}

    fn register_counter(&self, key: &Key, _: &Metadata<'_>) -> Counter {
        let labels = key
            .labels()
            .map(|label| (label.key().to_owned(), label.value().to_owned()))
            .collect();
        let mut counters = self.counters.lock().expect("test recorder lock");
        if let Some(counter) = counters
            .iter()
            .find(|counter| counter.name == key.name() && counter.labels == labels)
        {
            return Counter::from_arc(counter.value.clone());
        }
        let value = Arc::new(AtomicU64::new(0));
        counters.push(RegisteredCounter {
            name: key.name().to_owned(),
            labels,
            value: value.clone(),
        });
        Counter::from_arc(value)
    }

    fn register_gauge(&self, _: &Key, _: &Metadata<'_>) -> Gauge {
        Gauge::noop()
    }

    fn register_histogram(&self, _: &Key, _: &Metadata<'_>) -> Histogram {
        Histogram::noop()
    }
}

fn app_labels(namespace: &str, name: &str) -> Vec<(String, String)> {
    vec![
        (APP_METRIC_NAMESPACE_LABEL.to_owned(), namespace.to_owned()),
        (APP_METRIC_NAME_LABEL.to_owned(), name.to_owned()),
    ]
}

#[test]
fn reuses_registered_series_for_repeated_events() {
    let recorder = RecordingRecorder::default();
    let cache = Mutex::new(AppMetricCounterCache::default());
    let namespace = AppMetricNamespace::new("example").expect("valid namespace fixture");
    let other_namespace = AppMetricNamespace::new("other").expect("valid namespace fixture");
    let first = AppMetricName::new("first").expect("valid name fixture");
    let second = AppMetricName::new("second").expect("valid name fixture");

    with_local_recorder(&recorder, || {
        metric_counter(&cache, &namespace, &first).increment(1);
        metric_counter(&cache, &namespace, &first).increment(1);
        metric_counter(&cache, &namespace, &second).increment(1);
        metric_counter(&cache, &other_namespace, &first).increment(1);
    });

    assert_eq!(recorder.registrations(APP_EVENT_COUNTER), 3);
    assert_eq!(
        recorder.value(APP_EVENT_COUNTER, &app_labels("example", "first")),
        Some(2)
    );
    assert_eq!(
        recorder.value(APP_EVENT_COUNTER, &app_labels("example", "second")),
        Some(1)
    );
    assert_eq!(
        recorder.value(APP_EVENT_COUNTER, &app_labels("other", "first")),
        Some(1)
    );
    assert_eq!(
        recorder.registrations(APP_METRIC_SERIES_REJECTED_COUNTER),
        0
    );
}

#[test]
fn rejects_invalid_metric_names_before_registering_series() {
    let recorder = RecordingRecorder::default();

    with_local_recorder(&recorder, || {
        assert_eq!(
            record_app_metric_counter_by_name("example", "request-id"),
            Err(AppKitError::new(
                AppKitField::MetricName,
                AppKitErrorReason::InvalidCharacter,
            ))
        );
    });

    assert_eq!(recorder.registrations(APP_EVENT_COUNTER), 0);
}

#[test]
fn recreates_a_series_after_the_recorder_evicts_it() {
    let recorder = RecordingRecorder::default();
    let cache = Mutex::new(AppMetricCounterCache::default());
    let namespace = AppMetricNamespace::new("example").expect("valid namespace fixture");
    let name = AppMetricName::new("event").expect("valid name fixture");
    let labels = app_labels("example", "event");

    with_local_recorder(&recorder, || {
        metric_counter(&cache, &namespace, &name).increment(1);
        assert_eq!(recorder.value(APP_EVENT_COUNTER, &labels), Some(1));
        recorder.evict(APP_EVENT_COUNTER, &labels);
        metric_counter(&cache, &namespace, &name).increment(1);
    });

    assert_eq!(recorder.value(APP_EVENT_COUNTER, &labels), Some(1));
    assert_eq!(cache.lock().expect("test cache lock").series_count, 1);
}

#[test]
fn refuses_new_series_after_budget_without_disabling_existing_series() {
    let recorder = RecordingRecorder::default();
    let cache = Mutex::new(AppMetricCounterCache::default());
    let namespace = AppMetricNamespace::new("example").expect("valid namespace fixture");
    let first = AppMetricName::new("event_0").expect("valid name fixture");
    let rejected = AppMetricName::new("new_series").expect("valid name fixture");

    with_local_recorder(&recorder, || {
        for index in 0..MAX_APP_METRIC_SERIES {
            let name = AppMetricName::new(format!("event_{index}"))
                .expect("generated name is a valid fixture");
            metric_counter(&cache, &namespace, &name).increment(1);
        }
        metric_counter(&cache, &namespace, &rejected).increment(1);
        metric_counter(&cache, &namespace, &rejected).increment(1);
        metric_counter(&cache, &namespace, &first).increment(1);
    });

    assert_eq!(
        recorder.registrations(APP_EVENT_COUNTER),
        MAX_APP_METRIC_SERIES
    );
    assert_eq!(
        recorder.registrations(APP_METRIC_SERIES_REJECTED_COUNTER),
        1
    );
    assert_eq!(
        recorder.value(APP_EVENT_COUNTER, &app_labels("example", "event_0")),
        Some(2)
    );
    assert_eq!(
        recorder.value(APP_EVENT_COUNTER, &app_labels("example", "new_series")),
        None
    );
    assert_eq!(
        recorder.value(APP_METRIC_SERIES_REJECTED_COUNTER, &[]),
        Some(2)
    );

    let cache = cache.lock().expect("test cache lock");
    assert_eq!(cache.series_count, MAX_APP_METRIC_SERIES);
    let names = cache.series.get("example").expect("registered namespace");
    assert_eq!(names.len(), MAX_APP_METRIC_SERIES);
    assert!(!names.contains("new_series"));
}
