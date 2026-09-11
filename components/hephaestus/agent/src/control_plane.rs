// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Connect client for the central Hephaestus control plane.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use buffa::{DefaultInstance, EncodeSink, Message, MessageField};
use connectrpc::ErrorCode;
use connectrpc::client::{ClientConfig, HttpClient};
use http::HeaderValue;
use reallyme_hephaestus_contract::generated::connect::reallyme::hephaestus::v1::{
    HEPHAESTUS_SERVICE_REGISTER_AGENT_SPEC, HephaestusServiceClient,
};
use reallyme_hephaestus_contract::generated::proto::reallyme::domain::v1::{
    HephaestusAgentBootReport as ProtoAgentBootReport, HephaestusAgentReport as ProtoAgentReport,
    HephaestusCadvisorSummary as ProtoCadvisorSummary, HephaestusContainerHealthState,
    HephaestusContainerReport as ProtoContainerReport, HephaestusContainerRestartReason,
    HephaestusContainerState, HephaestusHostResourceReport as ProtoHostResourceReport,
    HephaestusHostServiceReport as ProtoHostServiceReport, HephaestusHostServiceState,
    HephaestusNodeExporterSummary as ProtoNodeExporterSummary,
    HephaestusObservabilityReport as ProtoObservabilityReport,
    HephaestusPatchStateReport as ProtoPatchStateReport,
    HephaestusRuntimeVersionReport as ProtoRuntimeVersionReport, HephaestusServiceProbeKind,
    HephaestusServiceProbeReport as ProtoServiceProbeReport, HephaestusServiceProbeStatus,
    HephaestusTailscaleReport as ProtoTailscaleReport, HephaestusUnattendedUpgradesState,
};
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1::__buffa::view::RegisterAgentResponseView;
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1::{
    CompleteAgentActionRequest, PollAgentActionsRequest, PollAgentActionsResponse,
    RegisterAgentRequest, RegisterAgentResponse, ResolveAgentSecretRequest,
    ResolveAgentSecretResponse, SubmitAgentReportRequest,
};
use reallyme_hephaestus_domain::{
    HephaestusAgentBootReport, HephaestusAgentBootSecret, HephaestusAgentPublicKey,
    HephaestusAgentPublicKeyFingerprint, HephaestusAgentReport, HephaestusAgentRuntimeToken,
    HephaestusContainerHealthState as DomainContainerHealthState,
    HephaestusContainerRestartReason as DomainContainerRestartReason,
    HephaestusContainerState as DomainContainerState,
    HephaestusHostServiceState as DomainHostServiceState,
};
use secrecy::SecretString;
use url::Url;
use zeroize::Zeroize;

use crate::actions::{AgentActionOutcome, AgentActionReceipt};
use crate::config::HephaestusAgentConfig;
use crate::docker_service::{DockerServiceSecretResolver, SecretRef};
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};

const AUTHORIZATION_HEADER: &str = "authorization";
const USER_AGENT_HEADER: &str = "user-agent";
const USER_AGENT_VALUE: &str = "reallyme-hephaestus-agent/0.1";
const BEARER_PREFIX: &str = "Bearer ";

#[derive(Clone, Default, PartialEq, serde::Serialize)]
#[serde(transparent)]
struct SensitiveRegisterAgentRequest(RegisterAgentRequest);

impl SensitiveRegisterAgentRequest {
    fn new(request: RegisterAgentRequest) -> Self {
        Self(request)
    }

    fn zeroize_bootstrap_token(&mut self) {
        self.0.bootstrap_token.zeroize();
    }
}

impl Drop for SensitiveRegisterAgentRequest {
    fn drop(&mut self) {
        self.zeroize_bootstrap_token();
    }
}

impl DefaultInstance for SensitiveRegisterAgentRequest {
    fn default_instance() -> &'static Self {
        static VALUE: std::sync::OnceLock<SensitiveRegisterAgentRequest> =
            std::sync::OnceLock::new();
        VALUE.get_or_init(Self::default)
    }
}

impl Message for SensitiveRegisterAgentRequest {
    fn compute_size(&self, cache: &mut buffa::SizeCache) -> u32 {
        self.0.compute_size(cache)
    }

    fn write_to(&self, cache: &mut buffa::SizeCache, buf: &mut impl EncodeSink) {
        self.0.write_to(cache, buf);
    }

    fn merge_field(
        &mut self,
        tag: buffa::encoding::Tag,
        buf: &mut impl buffa::bytes::Buf,
        depth: buffa::DecodeContext<'_>,
    ) -> Result<(), buffa::DecodeError> {
        self.0.merge_field(tag, buf, depth)
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

#[derive(Clone, Default, PartialEq, serde::Serialize)]
#[serde(transparent)]
struct SensitiveResolveAgentSecretResponse(ResolveAgentSecretResponse);

impl SensitiveResolveAgentSecretResponse {
    fn new(response: ResolveAgentSecretResponse) -> Self {
        Self(response)
    }

    fn into_secret_value(mut self) -> SecretString {
        SecretString::new(std::mem::take(&mut self.0.secret_value).into_boxed_str())
    }

    fn zeroize_secret_value(&mut self) {
        self.0.secret_value.zeroize();
    }
}

impl Drop for SensitiveResolveAgentSecretResponse {
    fn drop(&mut self) {
        self.zeroize_secret_value();
    }
}

impl DefaultInstance for SensitiveResolveAgentSecretResponse {
    fn default_instance() -> &'static Self {
        static VALUE: std::sync::OnceLock<SensitiveResolveAgentSecretResponse> =
            std::sync::OnceLock::new();
        VALUE.get_or_init(Self::default)
    }
}

impl Message for SensitiveResolveAgentSecretResponse {
    fn compute_size(&self, cache: &mut buffa::SizeCache) -> u32 {
        self.0.compute_size(cache)
    }

    fn write_to(&self, cache: &mut buffa::SizeCache, buf: &mut impl EncodeSink) {
        self.0.write_to(cache, buf);
    }

    fn merge_field(
        &mut self,
        tag: buffa::encoding::Tag,
        buf: &mut impl buffa::bytes::Buf,
        depth: buffa::DecodeContext<'_>,
    ) -> Result<(), buffa::DecodeError> {
        self.0.merge_field(tag, buf, depth)
    }

    fn clear(&mut self) {
        self.0.clear();
    }
}

/// Hephaestus outbound client. All communication uses generated Connect RPC.
#[derive(Clone)]
pub struct HephaestusControlPlaneClient {
    transport: HttpClient,
    config: ClientConfig,
    client: HephaestusServiceClient<HttpClient>,
}

impl HephaestusControlPlaneClient {
    /// Constructs an unauthenticated Connect client for first registration.
    pub fn for_registration(config: &HephaestusAgentConfig) -> AgentResult<Self> {
        Self::new(config, None)
    }

    /// Constructs an authenticated Connect client for registered agents.
    pub fn for_runtime(
        config: &HephaestusAgentConfig,
        runtime_token: &HephaestusAgentRuntimeToken,
    ) -> AgentResult<Self> {
        Self::new(config, Some(runtime_token))
    }

    fn new(
        config: &HephaestusAgentConfig,
        runtime_token: Option<&HephaestusAgentRuntimeToken>,
    ) -> AgentResult<Self> {
        let base_uri = config
            .controller_base_url()
            .as_str()
            .parse::<http::Uri>()
            .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidUrl))?;
        let user_agent = HeaderValue::from_static(USER_AGENT_VALUE);
        let mut client_config =
            ClientConfig::new(base_uri).with_default_header(USER_AGENT_HEADER, user_agent);
        if let Some(runtime_token) = runtime_token {
            let auth = bearer_header(runtime_token)?;
            client_config = client_config.with_default_header(AUTHORIZATION_HEADER, auth);
        }
        let transport = transport_for_url(config.controller_base_url())?;
        Ok(Self {
            client: HephaestusServiceClient::new(transport.clone(), client_config.clone()),
            transport,
            config: client_config,
        })
    }

    /// Registers the agent after first boot over Connect.
    pub async fn register_agent(
        &self,
        report: &HephaestusAgentBootReport,
        bootstrap_token: &HephaestusAgentBootSecret,
        agent_version: &str,
        agent_public_key: &HephaestusAgentPublicKey,
        agent_public_key_fingerprint: &HephaestusAgentPublicKeyFingerprint,
        agent_registration_signature: &str,
    ) -> AgentResult<RegisterAgentResponse> {
        let request = SensitiveRegisterAgentRequest::new(RegisterAgentRequest {
            report: MessageField::some(proto_boot_report(report)),
            bootstrap_token: bootstrap_token.expose_as_str().to_owned(),
            agent_version: agent_version.to_owned(),
            agent_public_key: agent_public_key.as_str().to_owned(),
            agent_public_key_fingerprint: agent_public_key_fingerprint.as_str().to_owned(),
            agent_registration_signature: agent_registration_signature.to_owned(),
            __buffa_unknown_fields: Default::default(),
        });
        connectrpc::client::call_unary::<_, _, RegisterAgentResponseView<'static>>(
            &self.transport,
            &self.config,
            HEPHAESTUS_SERVICE_REGISTER_AGENT_SPEC.with_origin(connectrpc::SpecOrigin::Client),
            request,
            connectrpc::client::CallOptions::default(),
        )
        .await
        .map(connectrpc::client::UnaryResponse::into_owned)
        .map_err(|error| connect_failed("register_agent", &error))
    }

    /// Submits one heartbeat report over Connect.
    pub async fn submit_agent_report(&self, report: &HephaestusAgentReport) -> AgentResult<()> {
        let request = SubmitAgentReportRequest {
            report: MessageField::some(proto_agent_report(report)),
            __buffa_unknown_fields: Default::default(),
        };
        self.client
            .submit_agent_report(request)
            .await
            .map_err(|error| connect_failed("submit_agent_report", &error))?;
        Ok(())
    }

    /// Polls central Hephaestus for bounded node-local actions.
    pub async fn poll_agent_actions(
        &self,
        node_id: &str,
        agent_version: &str,
        observed_generation: u64,
    ) -> AgentResult<PollAgentActionsResponse> {
        let request = PollAgentActionsRequest {
            node_id: node_id.to_owned(),
            agent_version: agent_version.to_owned(),
            observed_generation,
            __buffa_unknown_fields: Default::default(),
        };
        self.client
            .poll_agent_actions(request)
            .await
            .map(connectrpc::client::UnaryResponse::into_owned)
            .map_err(|error| connect_failed("poll_agent_actions", &error))
    }

    /// Reports completion of a previously polled action.
    pub async fn complete_agent_action(
        &self,
        receipt: &AgentActionReceipt,
        outcome: AgentActionOutcome,
    ) -> AgentResult<()> {
        let request = CompleteAgentActionRequest {
            node_id: receipt.node_id().to_owned(),
            action_id: receipt.action_id().to_owned(),
            idempotency_key: receipt.idempotency_key().to_owned(),
            status: buffa::EnumValue::from(outcome.status()),
            reason: buffa::EnumValue::from(outcome.reason()),
            completed_at_unix_secs: current_unix_seconds()?,
            __buffa_unknown_fields: Default::default(),
        };
        self.client
            .complete_agent_action(request)
            .await
            .map(|_response| ())
            .map_err(|error| connect_failed("complete_agent_action", &error))
    }
}

impl DockerServiceSecretResolver for HephaestusControlPlaneClient {
    fn resolve_agent_secret<'a>(
        &'a self,
        receipt: &'a AgentActionReceipt,
        secret_ref: &'a SecretRef,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = AgentResult<SecretString>> + Send + 'a>>
    {
        Box::pin(async move {
            let request = ResolveAgentSecretRequest {
                node_id: receipt.node_id().to_owned(),
                action_id: receipt.action_id().to_owned(),
                idempotency_key: receipt.idempotency_key().to_owned(),
                secret_ref: secret_ref.as_str().to_owned(),
                __buffa_unknown_fields: Default::default(),
            };
            let response = self
                .client
                .resolve_agent_secret(request)
                .await
                .map(connectrpc::client::UnaryResponse::into_owned)
                .map_err(|error| connect_failed("resolve_agent_secret", &error))?;
            let response = SensitiveResolveAgentSecretResponse::new(response);
            Ok(response.into_secret_value())
        })
    }
}

fn connect_failed(
    operation: &'static str,
    error: &connectrpc::ConnectError,
) -> HephaestusAgentError {
    tracing::warn!(
        operation,
        code = connect_error_code_label(error.code),
        message = connect_error_message_label(error.message.as_deref()),
        "hephaestus connect rpc failed"
    );
    HephaestusAgentError::new(HephaestusAgentErrorReason::ConnectFailed)
}

fn connect_error_message_label(message: Option<&str>) -> &'static str {
    let Some(message) = message else {
        return "none";
    };
    if message.starts_with("failed to decode response") {
        return "response_decode_failed";
    }
    if message.starts_with("unexpected content-type") {
        return "unexpected_content_type";
    }
    if message.starts_with("HTTP error") {
        return "http_error";
    }
    match message {
        "decode failed" => "decode_failed",
        "missing content-type" => "missing_content_type",
        "invalid content-type" => "invalid_content_type",
        "Hephaestus port is unavailable" => "hephaestus_port_unavailable",
        "Invalid request" => "invalid_request",
        "Hephaestus app is unavailable" => "hephaestus_app_unavailable",
        "Invalid health-check target" => "invalid_health_check_target",
        "Invalid system clock" => "invalid_system_clock",
        _ => "other",
    }
}

fn connect_error_code_label(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::Canceled => "canceled",
        ErrorCode::Unknown => "unknown",
        ErrorCode::InvalidArgument => "invalid_argument",
        ErrorCode::DeadlineExceeded => "deadline_exceeded",
        ErrorCode::NotFound => "not_found",
        ErrorCode::AlreadyExists => "already_exists",
        ErrorCode::PermissionDenied => "permission_denied",
        ErrorCode::ResourceExhausted => "resource_exhausted",
        ErrorCode::FailedPrecondition => "failed_precondition",
        ErrorCode::Aborted => "aborted",
        ErrorCode::OutOfRange => "out_of_range",
        ErrorCode::Unimplemented => "unimplemented",
        ErrorCode::Internal => "internal",
        ErrorCode::Unavailable => "unavailable",
        ErrorCode::DataLoss => "data_loss",
        ErrorCode::Unauthenticated => "unauthenticated",
        _ => "unknown",
    }
}

fn proto_boot_report(value: &HephaestusAgentBootReport) -> ProtoAgentBootReport {
    ProtoAgentBootReport {
        server_id: value.server_id().as_str().to_owned(),
        provider_server_id: value.provider_server_id().as_str().to_owned(),
        site_id: value.site_id().as_str().to_owned(),
        public_ip: value
            .public_ip()
            .map(reallyme_hephaestus_domain::HephaestusPublicIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        private_ip: value
            .private_ip()
            .map(reallyme_hephaestus_domain::HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        boot_time_unix_secs: value.boot_time_unix_secs(),
        generated_at_unix_secs: value.generated_at_unix_secs(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_agent_report(value: &HephaestusAgentReport) -> ProtoAgentReport {
    ProtoAgentReport {
        server_id: value.server_id().as_str().to_owned(),
        provider_server_id: value.provider_server_id().as_str().to_owned(),
        private_ip: value
            .private_ip()
            .map(reallyme_hephaestus_domain::HephaestusPrivateIpAddress::as_str)
            .unwrap_or_default()
            .to_owned(),
        generated_at_unix_secs: value.generated_at_unix_secs(),
        resources: MessageField::some(ProtoHostResourceReport {
            cpu_usage_basis_points: u32::from(value.resources().cpu_usage_basis_points()),
            memory_total_bytes: value.resources().memory_total_bytes(),
            memory_used_bytes: value.resources().memory_used_bytes(),
            disk_total_bytes: value.resources().disk_total_bytes(),
            disk_used_bytes: value.resources().disk_used_bytes(),
            __buffa_unknown_fields: Default::default(),
        }),
        containers: value
            .containers()
            .iter()
            .map(proto_container_report)
            .collect(),
        host_services: value
            .host_services()
            .iter()
            .map(proto_host_service_report)
            .collect(),
        actual_apps: value
            .actual_apps()
            .iter()
            .map(reallyme_hephaestus_domain::HephaestusServerApp::as_str)
            .map(str::to_owned)
            .collect(),
        tailscale: MessageField::some(proto_tailscale_report(value.tailscale())),
        runtime_versions: MessageField::some(proto_runtime_versions(value.runtime_versions())),
        patch_state: MessageField::some(proto_patch_state(value.patch_state())),
        observability: MessageField::some(proto_observability(value.observability())),
        service_probes: value
            .service_probes()
            .iter()
            .map(proto_service_probe_report)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_tailscale_report(
    value: &reallyme_hephaestus_domain::HephaestusTailscaleReport,
) -> ProtoTailscaleReport {
    ProtoTailscaleReport {
        hostname: value.hostname().unwrap_or_default().to_owned(),
        magic_dns_name: value.magic_dns_name().unwrap_or_default().to_owned(),
        ips: value.ips().to_vec(),
        tags: value.tags().to_vec(),
        services: value.services().to_vec(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_runtime_versions(
    value: &reallyme_hephaestus_domain::HephaestusRuntimeVersionReport,
) -> ProtoRuntimeVersionReport {
    ProtoRuntimeVersionReport {
        docker_version: value.docker_version().unwrap_or_default().to_owned(),
        docker_compose_version: value
            .docker_compose_version()
            .unwrap_or_default()
            .to_owned(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_patch_state(
    value: &reallyme_hephaestus_domain::HephaestusPatchStateReport,
) -> ProtoPatchStateReport {
    ProtoPatchStateReport {
        reboot_required: value.reboot_required(),
        unattended_upgrades_state: buffa::EnumValue::from(proto_unattended_state(
            value.unattended_upgrades_state(),
        ) as i32),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_observability(
    value: reallyme_hephaestus_domain::HephaestusObservabilityReport,
) -> ProtoObservabilityReport {
    ProtoObservabilityReport {
        cadvisor: MessageField::some(proto_cadvisor_summary(value.cadvisor())),
        node_exporter: MessageField::some(proto_node_exporter_summary(value.node_exporter())),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_cadvisor_summary(
    value: reallyme_hephaestus_domain::HephaestusCadvisorSummary,
) -> ProtoCadvisorSummary {
    ProtoCadvisorSummary {
        reachable: value.reachable(),
        container_metric_lines: value.container_metric_lines(),
        has_cpu_metrics: value.has_cpu_metrics(),
        has_memory_metrics: value.has_memory_metrics(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_node_exporter_summary(
    value: reallyme_hephaestus_domain::HephaestusNodeExporterSummary,
) -> ProtoNodeExporterSummary {
    ProtoNodeExporterSummary {
        reachable: value.reachable(),
        metric_lines: value.metric_lines(),
        has_cpu_metrics: value.has_cpu_metrics(),
        has_memory_metrics: value.has_memory_metrics(),
        has_filesystem_metrics: value.has_filesystem_metrics(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_service_probe_report(
    value: &reallyme_hephaestus_domain::HephaestusServiceProbeReport,
) -> ProtoServiceProbeReport {
    ProtoServiceProbeReport {
        probe_name: value.probe_name().to_owned(),
        service_name: value.service_name().to_owned(),
        kind: buffa::EnumValue::from(proto_service_probe_kind(value.kind()) as i32),
        target: value.target().to_owned(),
        status: buffa::EnumValue::from(proto_service_probe_status(value.status()) as i32),
        http_status_code: value.http_status_code().map(u32::from).unwrap_or_default(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_container_report(
    value: &reallyme_hephaestus_domain::HephaestusContainerReport,
) -> ProtoContainerReport {
    ProtoContainerReport {
        container_name: value.name().as_str().to_owned(),
        image: value.image().as_str().to_owned(),
        state: buffa::EnumValue::from(proto_container_state(value.state()) as i32),
        health_state: buffa::EnumValue::from(proto_container_health(value.health()) as i32),
        restart_required: value.restart_required(),
        restart_reason: buffa::EnumValue::from(proto_container_restart_reason(
            value.restart_reason(),
        ) as i32),
        recent_log_lines: value
            .recent_logs()
            .iter()
            .map(reallyme_hephaestus_domain::HephaestusAgentReportLine::as_str)
            .map(str::to_owned)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_host_service_report(
    value: &reallyme_hephaestus_domain::HephaestusHostServiceReport,
) -> ProtoHostServiceReport {
    ProtoHostServiceReport {
        unit_name: value.unit_name().as_str().to_owned(),
        state: buffa::EnumValue::from(proto_host_service_state(value.state()) as i32),
        recent_log_lines: value
            .recent_logs()
            .iter()
            .map(reallyme_hephaestus_domain::HephaestusAgentReportLine::as_str)
            .map(str::to_owned)
            .collect(),
        __buffa_unknown_fields: Default::default(),
    }
}

fn proto_container_state(value: DomainContainerState) -> HephaestusContainerState {
    match value {
        DomainContainerState::Created => {
            HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_CREATED
        }
        DomainContainerState::Running => {
            HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_RUNNING
        }
        DomainContainerState::Restarting => {
            HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_RESTARTING
        }
        DomainContainerState::Removing => {
            HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_REMOVING
        }
        DomainContainerState::Paused => HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_PAUSED,
        DomainContainerState::Exited => HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_EXITED,
        DomainContainerState::Dead => HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_DEAD,
        DomainContainerState::Unknown => {
            HephaestusContainerState::HEPHAESTUS_CONTAINER_STATE_UNKNOWN
        }
    }
}

fn proto_container_health(value: DomainContainerHealthState) -> HephaestusContainerHealthState {
    match value {
        DomainContainerHealthState::Healthy => {
            HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_HEALTHY
        }
        DomainContainerHealthState::Unhealthy => {
            HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNHEALTHY
        }
        DomainContainerHealthState::Starting => {
            HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_STARTING
        }
        DomainContainerHealthState::None => {
            HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_NONE
        }
        DomainContainerHealthState::Unknown => {
            HephaestusContainerHealthState::HEPHAESTUS_CONTAINER_HEALTH_STATE_UNKNOWN
        }
    }
}

fn proto_container_restart_reason(
    value: DomainContainerRestartReason,
) -> HephaestusContainerRestartReason {
    match value {
        DomainContainerRestartReason::None => {
            HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_NONE
        }
        DomainContainerRestartReason::Unhealthy => {
            HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNHEALTHY
        }
        DomainContainerRestartReason::Restarting => {
            HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_RESTARTING
        }
        DomainContainerRestartReason::Exited => {
            HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_EXITED
        }
        DomainContainerRestartReason::Dead => {
            HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_DEAD
        }
        DomainContainerRestartReason::Unknown => {
            HephaestusContainerRestartReason::HEPHAESTUS_CONTAINER_RESTART_REASON_UNKNOWN
        }
    }
}

fn proto_host_service_state(value: DomainHostServiceState) -> HephaestusHostServiceState {
    match value {
        DomainHostServiceState::Active => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_ACTIVE
        }
        DomainHostServiceState::Reloading => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_RELOADING
        }
        DomainHostServiceState::Inactive => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_INACTIVE
        }
        DomainHostServiceState::Failed => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_FAILED
        }
        DomainHostServiceState::Activating => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_ACTIVATING
        }
        DomainHostServiceState::Deactivating => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_DEACTIVATING
        }
        DomainHostServiceState::Unknown => {
            HephaestusHostServiceState::HEPHAESTUS_HOST_SERVICE_STATE_UNKNOWN
        }
    }
}

fn proto_unattended_state(
    value: reallyme_hephaestus_domain::HephaestusUnattendedUpgradesState,
) -> HephaestusUnattendedUpgradesState {
    match value {
        reallyme_hephaestus_domain::HephaestusUnattendedUpgradesState::Active => {
            HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_ACTIVE
        }
        reallyme_hephaestus_domain::HephaestusUnattendedUpgradesState::Inactive => {
            HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_INACTIVE
        }
        reallyme_hephaestus_domain::HephaestusUnattendedUpgradesState::Failed => {
            HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_FAILED
        }
        reallyme_hephaestus_domain::HephaestusUnattendedUpgradesState::NotInstalled => {
            HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_NOT_INSTALLED
        }
        reallyme_hephaestus_domain::HephaestusUnattendedUpgradesState::Unknown => {
            HephaestusUnattendedUpgradesState::HEPHAESTUS_UNATTENDED_UPGRADES_STATE_UNKNOWN
        }
    }
}

fn proto_service_probe_kind(
    value: reallyme_hephaestus_domain::HephaestusServiceProbeKind,
) -> HephaestusServiceProbeKind {
    match value {
        reallyme_hephaestus_domain::HephaestusServiceProbeKind::Http => {
            HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_HTTP
        }
        reallyme_hephaestus_domain::HephaestusServiceProbeKind::Tcp => {
            HephaestusServiceProbeKind::HEPHAESTUS_SERVICE_PROBE_KIND_TCP
        }
    }
}

fn proto_service_probe_status(
    value: reallyme_hephaestus_domain::HephaestusServiceProbeStatus,
) -> HephaestusServiceProbeStatus {
    match value {
        reallyme_hephaestus_domain::HephaestusServiceProbeStatus::Ok => {
            HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_OK
        }
        reallyme_hephaestus_domain::HephaestusServiceProbeStatus::Failed => {
            HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_FAILED
        }
        reallyme_hephaestus_domain::HephaestusServiceProbeStatus::Skipped => {
            HephaestusServiceProbeStatus::HEPHAESTUS_SERVICE_PROBE_STATUS_SKIPPED
        }
    }
}

fn transport_for_url(url: &Url) -> AgentResult<HttpClient> {
    match url.scheme() {
        "http" => Ok(HttpClient::plaintext()),
        "https" => {
            let mut roots = connectrpc::rustls::RootCertStore::empty();
            let result = rustls_native_certs::load_native_certs();
            for cert in result.certs {
                roots.add(cert).map_err(|_error| {
                    HephaestusAgentError::new(HephaestusAgentErrorReason::TlsUnavailable)
                })?;
            }
            if !result.errors.is_empty() || roots.is_empty() {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::TlsUnavailable,
                ));
            }
            let tls = connectrpc::rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            Ok(HttpClient::with_tls(Arc::new(tls)))
        }
        _ => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidUrl,
        )),
    }
}

fn bearer_header(token: &HephaestusAgentRuntimeToken) -> AgentResult<HeaderValue> {
    let mut value = String::with_capacity(
        BEARER_PREFIX
            .len()
            .checked_add(token.expose_as_str().len())
            .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))?,
    );
    value.push_str(BEARER_PREFIX);
    value.push_str(token.expose_as_str());
    let mut header = HeaderValue::from_str(value.as_str())
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidHeader))?;
    header.set_sensitive(true);
    value.zeroize();
    Ok(header)
}

fn current_unix_seconds() -> AgentResult<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidNumber))
}

#[cfg(test)]
mod tests;
