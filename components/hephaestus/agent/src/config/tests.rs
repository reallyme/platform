// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::time::Duration;

use super::{HephaestusAgentConfig, RawConfig, RawServiceProbeConfig};
use crate::error::HephaestusAgentErrorReason;

#[test]
fn validates_minimal_config() {
    let raw = RawConfig {
        controller_base_url: "http://heph.example.ts.net".to_owned(),
        server_id: "prod-nats-regional-ams-01".to_owned(),
        provider_server_id: "vultr-123".to_owned(),
        site_id: "ams".to_owned(),
        boot_secret: Some("boot-token".to_owned()),
        bootstrap_token_path: None,
        full_report_interval_seconds: None,
        action_poll_interval_seconds: None,
        command_timeout_seconds: None,
        state_dir: None,
        audit_log_path: None,
        cadvisor_metrics_url: None,
        node_exporter_metrics_url: None,
        observed_systemd_units: vec!["docker.service".to_owned()],
        observed_apps: vec!["nats".to_owned()],
        service_probes: vec![RawServiceProbeConfig {
            name: "nats-health".to_owned(),
            service_name: "nats".to_owned(),
            kind: "http".to_owned(),
            target: "http://127.0.0.1:8222/healthz".to_owned(),
        }],
    };

    let config = HephaestusAgentConfig::from_raw(raw);

    assert!(config.is_ok());
    let config = config.expect("validated config");
    assert_eq!(
        config.audit_log_path(),
        &PathBuf::from("/var/lib/reallyme/hephaestus-agent/action-audit.jsonl")
    );
    assert_eq!(config.full_report_interval(), Duration::from_secs(30));
    assert_eq!(config.action_poll_interval(), Duration::from_secs(2));
}

#[test]
fn rejects_plain_http_public_controller_url() {
    let raw = RawConfig {
        controller_base_url: "http://heph.example.com".to_owned(),
        server_id: "prod-nats-regional-ams-01".to_owned(),
        provider_server_id: "vultr-123".to_owned(),
        site_id: "ams".to_owned(),
        boot_secret: Some("boot-token".to_owned()),
        bootstrap_token_path: None,
        full_report_interval_seconds: None,
        action_poll_interval_seconds: None,
        command_timeout_seconds: None,
        state_dir: None,
        audit_log_path: None,
        cadvisor_metrics_url: None,
        node_exporter_metrics_url: None,
        observed_systemd_units: Vec::new(),
        observed_apps: Vec::new(),
        service_probes: Vec::new(),
    };

    let config = HephaestusAgentConfig::from_raw(raw);

    assert_eq!(
        config.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::InvalidUrl)
    );
}

#[test]
fn accepts_plain_http_private_controller_urls() {
    let new_raw = || RawConfig {
        controller_base_url: String::new(),
        server_id: "prod-nats-regional-ams-01".to_owned(),
        provider_server_id: "vultr-123".to_owned(),
        site_id: "ams".to_owned(),
        boot_secret: Some("boot-token".to_owned()),
        bootstrap_token_path: None,
        full_report_interval_seconds: None,
        action_poll_interval_seconds: None,
        command_timeout_seconds: None,
        state_dir: None,
        audit_log_path: None,
        cadvisor_metrics_url: None,
        node_exporter_metrics_url: None,
        observed_systemd_units: Vec::new(),
        observed_apps: Vec::new(),
        service_probes: Vec::new(),
    };

    for controller_base_url in [
        "http://heph.example.ts.net",
        "http://100.100.100.100",
        "http://10.0.0.10",
        "http://192.168.1.10",
        "http://127.0.0.1:8080",
    ] {
        let mut raw = new_raw();
        raw.controller_base_url = controller_base_url.to_owned();

        assert!(HephaestusAgentConfig::from_raw(raw).is_ok());
    }
}

#[test]
fn rejects_plain_http_lookalike_private_controller_urls() {
    let new_raw = || RawConfig {
        controller_base_url: String::new(),
        server_id: "prod-nats-regional-ams-01".to_owned(),
        provider_server_id: "vultr-123".to_owned(),
        site_id: "ams".to_owned(),
        boot_secret: Some("boot-token".to_owned()),
        bootstrap_token_path: None,
        full_report_interval_seconds: None,
        action_poll_interval_seconds: None,
        command_timeout_seconds: None,
        state_dir: None,
        audit_log_path: None,
        cadvisor_metrics_url: None,
        node_exporter_metrics_url: None,
        observed_systemd_units: Vec::new(),
        observed_apps: Vec::new(),
        service_probes: Vec::new(),
    };

    for controller_base_url in [
        "http://100.128.0.1",
        "http://169.254.1.1",
        "http://8.8.8.8",
        "http://[2001:db8::1]",
        "http://ts.net",
    ] {
        let mut raw = new_raw();
        raw.controller_base_url = controller_base_url.to_owned();

        let config = HephaestusAgentConfig::from_raw(raw);

        assert_eq!(
            config.err().map(|error| error.reason()),
            Some(HephaestusAgentErrorReason::InvalidUrl)
        );
    }
}

#[test]
fn rejects_nonlocal_cadvisor_url() {
    let raw = RawConfig {
        controller_base_url: "https://heph.example.ts.net".to_owned(),
        server_id: "prod-nats-regional-ams-01".to_owned(),
        provider_server_id: "vultr-123".to_owned(),
        site_id: "ams".to_owned(),
        boot_secret: Some("boot-token".to_owned()),
        bootstrap_token_path: None,
        full_report_interval_seconds: None,
        action_poll_interval_seconds: None,
        command_timeout_seconds: None,
        state_dir: None,
        audit_log_path: None,
        cadvisor_metrics_url: Some("http://example.com/metrics".to_owned()),
        node_exporter_metrics_url: None,
        observed_systemd_units: Vec::new(),
        observed_apps: Vec::new(),
        service_probes: Vec::new(),
    };

    let config = HephaestusAgentConfig::from_raw(raw);

    assert_eq!(
        config.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::InvalidUrl)
    );
}

#[test]
fn rejects_localhost_local_http_urls() {
    let raw = RawConfig {
        controller_base_url: "https://heph.example.ts.net".to_owned(),
        server_id: "prod-nats-regional-ams-01".to_owned(),
        provider_server_id: "vultr-123".to_owned(),
        site_id: "ams".to_owned(),
        boot_secret: Some("boot-token".to_owned()),
        bootstrap_token_path: None,
        full_report_interval_seconds: None,
        action_poll_interval_seconds: None,
        command_timeout_seconds: None,
        state_dir: None,
        audit_log_path: None,
        cadvisor_metrics_url: Some("http://localhost:8080/metrics".to_owned()),
        node_exporter_metrics_url: None,
        observed_systemd_units: Vec::new(),
        observed_apps: Vec::new(),
        service_probes: Vec::new(),
    };

    let config = HephaestusAgentConfig::from_raw(raw);

    assert_eq!(
        config.err().map(|error| error.reason()),
        Some(HephaestusAgentErrorReason::InvalidUrl)
    );
}
