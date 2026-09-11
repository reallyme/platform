// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    ServeCommandRunner, ServeConfig, ServeServiceConfig, TailscaleServiceDefinition,
    TailscaleServiceEndpoint, apply_service_configs_with_runner, read_serve_config,
    render_serve_config,
};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use std::collections::BTreeMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

struct PartialAdvertiseFailureRunner {
    advertised: Mutex<Vec<String>>,
}

impl PartialAdvertiseFailureRunner {
    fn new() -> Self {
        Self {
            advertised: Mutex::new(Vec::new()),
        }
    }

    fn advertised_services(&self) -> Vec<String> {
        self.advertised
            .lock()
            .expect("test advertised lock should not be poisoned")
            .clone()
    }
}

impl ServeCommandRunner for PartialAdvertiseFailureRunner {
    fn set_config<'a>(
        &'a self,
        _path: &'a Path,
        _command_timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(async { Ok(()) })
    }

    fn advertise<'a>(
        &'a self,
        service_name: &'a str,
        _command_timeout: Duration,
    ) -> Pin<Box<dyn Future<Output = AgentResult<()>> + Send + 'a>> {
        Box::pin(async move {
            self.advertised
                .lock()
                .expect("test advertised lock should not be poisoned")
                .push(service_name.to_owned());
            if service_name == "svc:typesense" {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::CommandFailed,
                ));
            }
            Ok(())
        })
    }
}

#[test]
fn renders_tailscale_service_config() {
    let endpoint =
        TailscaleServiceEndpoint::new("tcp:4222".to_owned(), "tcp://127.0.0.1:4222".to_owned())
            .unwrap_or_else(|error| panic!("valid endpoint: {error:?}"));
    let definition = TailscaleServiceDefinition::new("svc:nats-eu".to_owned(), vec![endpoint])
        .unwrap_or_else(|error| panic!("valid service: {error:?}"));
    let mut config = ServeConfig::empty();
    config.services.insert(
        definition.service_name().to_owned(),
        ServeServiceConfig::from_definition(&definition),
    );

    let rendered =
        render_serve_config(&config).unwrap_or_else(|error| panic!("rendered: {error:?}"));

    assert!(rendered.contains("\"svc:nats-eu\""));
    assert!(rendered.contains("\"tcp:4222\""));
    assert!(rendered.contains("\"tcp://127.0.0.1:4222\""));
}

#[test]
fn rejects_localhost_tailscale_service_upstream() {
    let result =
        TailscaleServiceEndpoint::new("tcp:4222".to_owned(), "tcp://localhost:4222".to_owned());

    assert!(result.is_err());
}

#[test]
fn service_config_is_deterministic() {
    let mut config = ServeConfig::empty();
    let mut endpoints = BTreeMap::new();
    endpoints.insert("tcp:4222".to_owned(), "tcp://127.0.0.1:4222".to_owned());
    config
        .services
        .insert("svc:nats-eu".to_owned(), ServeServiceConfig { endpoints });

    let first = render_serve_config(&config).unwrap_or_else(|error| panic!("rendered: {error:?}"));
    let second = render_serve_config(&config).unwrap_or_else(|error| panic!("rendered: {error:?}"));

    assert_eq!(first, second);
}

#[test]
fn service_status_name_collection_has_depth_cap() {
    let mut value = serde_json::Value::String("svc:too-deep".to_owned());
    for _index in 0..20 {
        value = serde_json::json!({ "nested": value });
    }
    let mut services = Vec::new();

    super::collect_service_names(&value, &mut services, 0);

    assert!(services.is_empty());
}

#[tokio::test]
async fn apply_service_configs_records_partial_advertise_failure() {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error:?}"));
    let path = dir.path().join("serveconfig.json");
    let runner = PartialAdvertiseFailureRunner::new();
    let nats = TailscaleServiceDefinition::new(
        "svc:nats-eu".to_owned(),
        vec![
            TailscaleServiceEndpoint::new("tcp:4222".to_owned(), "tcp://127.0.0.1:4222".to_owned())
                .unwrap_or_else(|error| panic!("valid endpoint: {error:?}")),
        ],
    )
    .unwrap_or_else(|error| panic!("valid service: {error:?}"));
    let typesense = TailscaleServiceDefinition::new(
        "svc:typesense".to_owned(),
        vec![
            TailscaleServiceEndpoint::new("tcp:8108".to_owned(), "tcp://127.0.0.1:8108".to_owned())
                .unwrap_or_else(|error| panic!("valid endpoint: {error:?}")),
        ],
    )
    .unwrap_or_else(|error| panic!("valid service: {error:?}"));

    let result = apply_service_configs_with_runner(
        &path,
        &runner,
        &[nats, typesense],
        Duration::from_secs(1),
    )
    .await;

    assert_eq!(
        result.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::CommandFailed)
    );
    assert_eq!(
        runner.advertised_services(),
        vec![String::from("svc:nats-eu"), String::from("svc:typesense")]
    );
    let installed = read_serve_config(path.as_path())
        .await
        .unwrap_or_else(|error| panic!("read serve config: {error:?}"));
    assert!(installed.services.contains_key("svc:nats-eu"));
    assert!(installed.services.contains_key("svc:typesense"));
}
