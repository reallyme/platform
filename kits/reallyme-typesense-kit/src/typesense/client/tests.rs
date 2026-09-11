// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use metrics::with_local_recorder;
use metrics_util::debugging::{DebugValue, Snapshotter};
use reqwest::Client;
use secrecy::SecretString;
use tokio::runtime::Builder;

use super::{
    CollectionSchemaRequest, LABEL_OPERATION, METRIC_REQUESTS_IN_FLIGHT, RequestKind,
    SearchRequest, TypesenseClient,
};
use crate::typesense::{
    CollectionField, CollectionName, FieldKind, FilterValue, PageSize, SearchFieldName,
    SearchFields, SearchFilter, SearchQuery, SortField, TypesenseConfig, TypesenseEndpoint,
    TypesenseEndpointSelection, TypesenseError, TypesenseTransportReason,
};

fn metric_gauge_value(
    snapshotter: &Snapshotter,
    metric_name: &str,
    operation: &str,
) -> Option<f64> {
    snapshotter
        .snapshot()
        .into_vec()
        .into_iter()
        .find_map(|(key, _, _, value)| {
            let operation_matches = key
                .key()
                .labels()
                .any(|label| label.key() == LABEL_OPERATION && label.value() == operation);
            if key.key().name() == metric_name && operation_matches {
                if let DebugValue::Gauge(value) = value {
                    Some(value.into_inner())
                } else {
                    None
                }
            } else {
                None
            }
        })
}

#[test]
fn collection_schema_request_omits_alias_field() -> Result<(), TypesenseError> {
    let schema = crate::typesense::CollectionSchema::new(
        CollectionName::parse("people_directory")?,
        vec![CollectionField::new(
            SearchFieldName::parse("handle")?,
            FieldKind::String,
        )],
        Some(SortField::from_search_field(SearchFieldName::parse(
            "handle",
        )?)),
    )?
    .with_alias(CollectionName::parse("directory")?);

    let encoded = serde_json::to_value(CollectionSchemaRequest::from(&schema)).map_err(|_| {
        TypesenseError::Transport {
            reason: TypesenseTransportReason::RequestSerializationFailed,
        }
    })?;

    let object = encoded.as_object().ok_or(TypesenseError::Transport {
        reason: TypesenseTransportReason::InvalidResponseBody,
    })?;

    assert!(!object.contains_key("alias"));
    assert!(object.contains_key("name"));
    assert!(object.contains_key("fields"));
    Ok(())
}

#[test]
fn execute_with_retries_clears_in_flight_gauge_on_early_return() -> Result<(), TypesenseError> {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| TypesenseError::Transport {
            reason: TypesenseTransportReason::RequestFailed,
        })?;
    let recorder = metrics_util::debugging::DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    let client = TypesenseClient {
        http_client: Client::new(),
        endpoints: Vec::new(),
        endpoint_selection: TypesenseEndpointSelection::NearestNode,
        endpoint_cursor: AtomicUsize::new(0),
        search_request_timeout: Duration::from_millis(50),
        import_request_timeout: Duration::from_millis(50),
        max_retries: 0,
        retry_initial_delay: Duration::from_millis(1),
        retry_max_delay: Duration::from_millis(1),
        retry_jitter_percent: 0,
        retry_entropy: AtomicU64::new(0),
    };

    with_local_recorder(&recorder, || {
        runtime.block_on(async {
            let result = client
                .execute_with_retries(
                    Duration::from_millis(50),
                    |endpoint| client.http_client.get(endpoint.as_str()),
                    RequestKind::Search,
                )
                .await;

            assert!(matches!(
                result,
                Err(TypesenseError::Transport {
                    reason: TypesenseTransportReason::RequestFailed
                })
            ));
        });
    });

    assert_eq!(
        metric_gauge_value(
            &snapshotter,
            METRIC_REQUESTS_IN_FLIGHT,
            RequestKind::Search.as_str()
        ),
        Some(0.0)
    );
    Ok(())
}

#[test]
fn search_request_omits_filter_when_caller_does_not_supply_one() -> Result<(), TypesenseError> {
    let query = SearchQuery::parse("alice")?;
    let query_by = SearchFields::new(vec![SearchFieldName::parse("handle")?])?;
    let page_size = PageSize::parse(10, PageSize::MAX_SEARCH_AS_YOU_TYPE)?;

    let request = SearchRequest::search_as_you_type(&query, &query_by, page_size, None, None);

    let encoded = serde_json::to_value(&request).map_err(|_| TypesenseError::Transport {
        reason: TypesenseTransportReason::RequestSerializationFailed,
    })?;
    let object = encoded.as_object().ok_or(TypesenseError::Transport {
        reason: TypesenseTransportReason::InvalidResponseBody,
    })?;

    assert!(!object.contains_key("filter_by"));
    Ok(())
}

#[test]
fn search_request_preserves_caller_owned_filter() -> Result<(), TypesenseError> {
    let query = SearchQuery::parse("alice")?;
    let query_by = SearchFields::new(vec![SearchFieldName::parse("handle")?])?;
    let page_size = PageSize::parse(10, PageSize::MAX_SEARCH_AS_YOU_TYPE)?;
    let filter = SearchFilter::exact(SearchFieldName::parse("verified")?, FilterValue::Bool(true));

    let request =
        SearchRequest::search_exact_match(&query, &query_by, page_size, None, Some(&filter));

    let encoded = serde_json::to_value(&request).map_err(|_| TypesenseError::Transport {
        reason: TypesenseTransportReason::RequestSerializationFailed,
    })?;
    let object = encoded.as_object().ok_or(TypesenseError::Transport {
        reason: TypesenseTransportReason::InvalidResponseBody,
    })?;

    assert_eq!(
        object.get("filter_by").and_then(serde_json::Value::as_str),
        Some("verified:true")
    );
    assert_eq!(
        object.get("prefix").and_then(serde_json::Value::as_bool),
        Some(false)
    );
    Ok(())
}

#[test]
fn new_client_seeds_retry_entropy_non_zero() -> Result<(), TypesenseError> {
    let endpoint = TypesenseEndpoint::parse("https://typesense.internal").map_err(|_| {
        TypesenseError::Transport {
            reason: TypesenseTransportReason::RequestFailed,
        }
    })?;
    let config = TypesenseConfig::new(
        endpoint,
        SecretString::new(String::from("test-key").into()),
        Duration::from_secs(1),
    )
    .map_err(|_| TypesenseError::Transport {
        reason: TypesenseTransportReason::RequestFailed,
    })?;
    let client = TypesenseClient::new(Client::new(), config);

    assert_ne!(client.retry_entropy.load(Ordering::Relaxed), 0);
    Ok(())
}

#[test]
fn nearest_node_selection_fails_over_across_retry_attempts() {
    let client = TypesenseClient {
        http_client: Client::new(),
        endpoints: Vec::new(),
        endpoint_selection: TypesenseEndpointSelection::NearestNode,
        endpoint_cursor: AtomicUsize::new(0),
        search_request_timeout: Duration::from_millis(50),
        import_request_timeout: Duration::from_millis(50),
        max_retries: 0,
        retry_initial_delay: Duration::from_millis(1),
        retry_max_delay: Duration::from_millis(1),
        retry_jitter_percent: 0,
        retry_entropy: AtomicU64::new(1),
    };

    assert_eq!(client.start_endpoint_index(3), 0);
    assert_eq!(client.endpoint_index(0, 0), 0);
    assert_eq!(client.endpoint_index(0, 1), 1);
    assert_eq!(client.endpoint_index(0, 2), 2);
}
