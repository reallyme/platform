// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed FoundationDB host configuration executed by the node-local agent.
//!
//! This module deliberately configures only the fixed FoundationDB service
//! surface used by ReallyMe hosts. It is not a generic file writer or command
//! runner; Hephaestus sends cluster topology data, and the agent renders the
//! small set of known FoundationDB files from validated fields.

use std::fs;
use std::net::IpAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as pb;
use serde_json::Value;
use thiserror::Error;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

const FOUNDATIONDB_CLUSTER_DIR: &str = "/etc/foundationdb";
const FOUNDATIONDB_CLUSTER_FILE: &str = "/etc/foundationdb/fdb.cluster";
const FOUNDATIONDB_CONFIG_FILE: &str = "/etc/foundationdb/foundationdb.conf";
const FOUNDATIONDB_DATA_DIR: &str = "/var/lib/foundationdb/data/4500";
const FOUNDATIONDB_LOG_DIR: &str = "/var/log/foundationdb";
const FOUNDATIONDB_UNIT: &str = "foundationdb";
const FDBSERVER_BINARY: &str = "/usr/sbin/fdbserver";
const FDBCLI_BINARY: &str = "/usr/bin/fdbcli";
const SYSTEMCTL_BINARY: &str = "/bin/systemctl";
const FOUNDATIONDB_LISTEN_ADDRESS: &str = "public";
const FOUNDATIONDB_RESTART_DELAY_SECONDS: u32 = 60;
const MAX_COORDINATORS: usize = 32;
const MAX_TOKEN_LEN: usize = 64;
const MAX_ID_LEN: usize = 64;
const MIN_PORT: u32 = 1;
const MAX_PORT: u32 = 65_535;
const FOUNDATIONDB_LOCAL_COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
const FOUNDATIONDB_STATUS_RETRY_ATTEMPTS: usize = 6;
const FOUNDATIONDB_CONFIGURE_RETRY_ATTEMPTS: usize = 12;
const FOUNDATIONDB_STATUS_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Typed FoundationDB service configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationDbServiceSpec {
    cluster_description: String,
    cluster_token: String,
    coordinators: Vec<FoundationDbCoordinator>,
    public_address: String,
    listen_port: u16,
    datacenter_id: String,
    zone_id: String,
    machine_id: String,
    process_class: FoundationDbProcessClass,
    redundancy_mode: FoundationDbRedundancyMode,
    storage_engine: FoundationDbStorageEngine,
    initialize_database: bool,
    cluster_operation: FoundationDbClusterOperation,
}

impl FoundationDbServiceSpec {
    /// Validates a protobuf FoundationDB action into an executable local spec.
    pub fn from_proto(
        action: pb::AgentFoundationDbServiceAction,
    ) -> Result<Self, FoundationDbServiceError> {
        let listen_port = validate_port(action.listen_port)?;
        let coordinators = action
            .coordinators
            .into_iter()
            .map(FoundationDbCoordinator::from_proto)
            .collect::<Result<Vec<_>, _>>()?;
        if coordinators.is_empty() || coordinators.len() > MAX_COORDINATORS {
            return Err(FoundationDbServiceError::InvalidTopology);
        }
        let cluster_operation = FoundationDbClusterOperation::from_proto(
            action
                .cluster_operation
                .as_known()
                .ok_or(FoundationDbServiceError::InvalidClusterOperation)?,
        )?;
        if action.initialize_database && cluster_operation != FoundationDbClusterOperation::Create {
            return Err(FoundationDbServiceError::InvalidClusterOperation);
        }
        Ok(Self {
            cluster_description: validate_cluster_token(action.cluster_description)?,
            cluster_token: validate_cluster_token(action.cluster_token)?,
            coordinators,
            public_address: validate_ip_address(action.public_address)?,
            listen_port,
            datacenter_id: validate_locality_id(action.datacenter_id)?,
            zone_id: validate_locality_id(action.zone_id)?,
            machine_id: validate_locality_id(action.machine_id)?,
            process_class: FoundationDbProcessClass::parse(action.process_class.as_str())?,
            redundancy_mode: FoundationDbRedundancyMode::parse(action.redundancy_mode.as_str())?,
            storage_engine: FoundationDbStorageEngine::parse(action.storage_engine.as_str())?,
            initialize_database: action.initialize_database,
            cluster_operation,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FoundationDbClusterOperation {
    Create,
    AddNode,
    Reconfigure,
}

impl FoundationDbClusterOperation {
    fn from_proto(
        value: pb::AgentFoundationDbClusterOperation,
    ) -> Result<Self, FoundationDbServiceError> {
        match value {
            pb::AgentFoundationDbClusterOperation::AGENT_FOUNDATION_DB_CLUSTER_OPERATION_CREATE_CLUSTER => {
                Ok(Self::Create)
            }
            pb::AgentFoundationDbClusterOperation::AGENT_FOUNDATION_DB_CLUSTER_OPERATION_JOIN_EXISTING_CLUSTER => {
                Ok(Self::AddNode)
            }
            pb::AgentFoundationDbClusterOperation::AGENT_FOUNDATION_DB_CLUSTER_OPERATION_RECONFIGURE_CLUSTER => {
                Ok(Self::Reconfigure)
            }
            pb::AgentFoundationDbClusterOperation::AGENT_FOUNDATION_DB_CLUSTER_OPERATION_UNSPECIFIED => {
                Err(FoundationDbServiceError::InvalidClusterOperation)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FoundationDbCoordinator {
    address: String,
    port: u16,
}

impl FoundationDbCoordinator {
    fn from_proto(
        value: pb::AgentFoundationDbCoordinator,
    ) -> Result<Self, FoundationDbServiceError> {
        Ok(Self {
            address: validate_ip_address(value.address)?,
            port: validate_port(value.port)?,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FoundationDbProcessClass {
    Storage,
    Stateless,
    Log,
    Coordinator,
}

impl FoundationDbProcessClass {
    fn parse(value: &str) -> Result<Self, FoundationDbServiceError> {
        match value {
            "storage" => Ok(Self::Storage),
            "stateless" => Ok(Self::Stateless),
            "log" => Ok(Self::Log),
            "coordinator" => Ok(Self::Coordinator),
            _ => Err(FoundationDbServiceError::InvalidProcessClass),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Storage => "storage",
            Self::Stateless => "stateless",
            Self::Log => "log",
            Self::Coordinator => "coordinator",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FoundationDbRedundancyMode {
    Single,
    Double,
    Triple,
}

impl FoundationDbRedundancyMode {
    fn parse(value: &str) -> Result<Self, FoundationDbServiceError> {
        match value {
            "single" => Ok(Self::Single),
            "double" => Ok(Self::Double),
            "triple" => Ok(Self::Triple),
            _ => Err(FoundationDbServiceError::InvalidRedundancyMode),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Double => "double",
            Self::Triple => "triple",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FoundationDbStorageEngine {
    Ssd2,
}

impl FoundationDbStorageEngine {
    fn parse(value: &str) -> Result<Self, FoundationDbServiceError> {
        match value {
            "ssd-2" => Ok(Self::Ssd2),
            _ => Err(FoundationDbServiceError::InvalidStorageEngine),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Ssd2 => "ssd-2",
        }
    }
}

/// FoundationDB service configuration failure reason.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum FoundationDbServiceError {
    /// The action contains an invalid IP address or port.
    #[error("invalid FoundationDB network address")]
    InvalidAddress,
    /// The action contains no coordinators or too many coordinators.
    #[error("invalid FoundationDB topology")]
    InvalidTopology,
    /// The action contains an invalid cluster token or locality identifier.
    #[error("invalid FoundationDB identifier")]
    InvalidIdentifier,
    /// The action contains an unsupported process class.
    #[error("invalid FoundationDB process class")]
    InvalidProcessClass,
    /// The action contains an unsupported redundancy mode.
    #[error("invalid FoundationDB redundancy mode")]
    InvalidRedundancyMode,
    /// The action contains an unsupported storage engine.
    #[error("invalid FoundationDB storage engine")]
    InvalidStorageEngine,
    /// The action requested an unsupported cluster lifecycle operation.
    #[error("invalid FoundationDB cluster operation")]
    InvalidClusterOperation,
    /// The agent could not create or write the fixed FoundationDB files.
    #[error("FoundationDB filesystem operation failed")]
    Filesystem,
    /// A fixed local FoundationDB command failed or timed out.
    #[error("FoundationDB local command failed")]
    LocalCommand,
    /// The agent could not start the FoundationDB service unit.
    #[error("FoundationDB service start failed")]
    ServiceStartFailed,
    /// The agent could not initialize a new FoundationDB database.
    #[error("FoundationDB database initialization failed")]
    ConfigureNewFailed,
    /// The agent could not observe a usable FoundationDB status.
    #[error("FoundationDB status unavailable")]
    StatusUnavailable,
    /// FoundationDB reported that coordination servers could not be reached.
    #[error("FoundationDB coordinators unavailable")]
    CoordinatorsUnavailable,
    /// FoundationDB reported that the database has not been initialized.
    #[error("FoundationDB database not configured")]
    DatabaseNotConfigured,
    /// FoundationDB reported that the database is currently unavailable.
    #[error("FoundationDB database unavailable")]
    DatabaseUnavailable,
    /// FoundationDB rejected the requested database configuration.
    #[error("FoundationDB configuration invalid")]
    ConfigurationInvalid,
    /// The local cluster file does not match the intended coordinator set.
    #[error("FoundationDB cluster file is stale")]
    StaleClusterFile,
    /// The local process has not appeared in cluster status yet.
    #[error("FoundationDB recruitment pending")]
    RecruitmentPending,
    /// FoundationDB recovery has not converged yet.
    #[error("FoundationDB recovery in progress")]
    RecoveryInProgress,
    /// FoundationDB data movement has not converged yet.
    #[error("FoundationDB data movement in progress")]
    DataMovementInProgress,
    /// FoundationDB coordinator quorum is not reachable.
    #[error("FoundationDB coordinator quorum lost")]
    CoordinatorQuorumLost,
    /// Observed storage process count cannot support the requested redundancy mode.
    #[error("FoundationDB insufficient storage for redundancy")]
    InsufficientStorageForRedundancy,
}

/// Renders FoundationDB configuration, starts the service, and optionally
/// initializes a brand-new database.
pub async fn configure_foundationdb_service(
    service: &FoundationDbServiceSpec,
    command_timeout: Duration,
) -> Result<(), FoundationDbServiceError> {
    let local_command_timeout = foundationdb_command_timeout(command_timeout);
    write_foundationdb_files(service)?;
    run_fixed_command(
        SYSTEMCTL_BINARY,
        ["start", FOUNDATIONDB_UNIT],
        local_command_timeout,
    )
    .await
    .map_err(|_error| FoundationDbServiceError::ServiceStartFailed)?;
    if service.cluster_operation == FoundationDbClusterOperation::Create {
        configure_new_database(service, local_command_timeout).await?;
    }
    if service.cluster_operation == FoundationDbClusterOperation::Reconfigure {
        configure_existing_database(service, local_command_timeout).await?;
    }
    wait_for_foundationdb_convergence(
        service,
        local_command_timeout,
        FOUNDATIONDB_STATUS_RETRY_ATTEMPTS,
        FOUNDATIONDB_STATUS_RETRY_DELAY,
    )
    .await
}

fn write_foundationdb_files(
    service: &FoundationDbServiceSpec,
) -> Result<(), FoundationDbServiceError> {
    create_dir(FOUNDATIONDB_CLUSTER_DIR, 0o755)?;
    write_file(
        FOUNDATIONDB_CLUSTER_FILE,
        render_cluster_file(service).as_str(),
        0o644,
    )?;
    write_file(
        FOUNDATIONDB_CONFIG_FILE,
        render_config_file(service).as_str(),
        0o644,
    )?;
    verify_cluster_file(service)?;
    Ok(())
}

fn verify_cluster_file(service: &FoundationDbServiceSpec) -> Result<(), FoundationDbServiceError> {
    let expected = render_cluster_file(service);
    let actual = fs::read_to_string(Path::new(FOUNDATIONDB_CLUSTER_FILE))
        .map_err(|_error| FoundationDbServiceError::Filesystem)?;
    if actual != expected {
        return Err(FoundationDbServiceError::StaleClusterFile);
    }
    Ok(())
}

fn create_dir(path: &str, mode: u32) -> Result<(), FoundationDbServiceError> {
    let existed = Path::new(path).exists();
    fs::create_dir_all(Path::new(path)).map_err(|_error| FoundationDbServiceError::Filesystem)?;
    if existed {
        return Ok(());
    }
    fs::set_permissions(Path::new(path), fs::Permissions::from_mode(mode))
        .map_err(|_error| FoundationDbServiceError::Filesystem)
}

fn write_file(path: &str, contents: &str, mode: u32) -> Result<(), FoundationDbServiceError> {
    fs::write(Path::new(path), contents.as_bytes())
        .map_err(|_error| FoundationDbServiceError::Filesystem)?;
    fs::set_permissions(Path::new(path), fs::Permissions::from_mode(mode))
        .map_err(|_error| FoundationDbServiceError::Filesystem)
}

fn render_cluster_file(service: &FoundationDbServiceSpec) -> String {
    let mut output = String::new();
    output.push_str(service.cluster_description.as_str());
    output.push(':');
    output.push_str(service.cluster_token.as_str());
    output.push('@');
    for (index, coordinator) in service.coordinators.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(coordinator.address.as_str());
        output.push(':');
        output.push_str(coordinator.port.to_string().as_str());
    }
    output.push('\n');
    output
}

fn render_config_file(service: &FoundationDbServiceSpec) -> String {
    let mut output = String::new();
    output.push_str("[fdbmonitor]\n");
    output.push_str("user = foundationdb\n");
    output.push_str("group = foundationdb\n");
    output.push_str("\n[general]\n");
    output.push_str("restart-delay = ");
    output.push_str(FOUNDATIONDB_RESTART_DELAY_SECONDS.to_string().as_str());
    output.push('\n');
    output.push_str("cluster-file = ");
    output.push_str(FOUNDATIONDB_CLUSTER_FILE);
    output.push_str("\n\n[fdbserver]\n");
    output.push_str("command = ");
    output.push_str(FDBSERVER_BINARY);
    output.push('\n');
    output.push_str("public-address = ");
    output.push_str(service.public_address.as_str());
    output.push(':');
    output.push_str(service.listen_port.to_string().as_str());
    output.push('\n');
    output.push_str("listen-address = ");
    output.push_str(FOUNDATIONDB_LISTEN_ADDRESS);
    output.push('\n');
    output.push_str("datadir = ");
    output.push_str(FOUNDATIONDB_DATA_DIR);
    output.push('\n');
    output.push_str("logdir = ");
    output.push_str(FOUNDATIONDB_LOG_DIR);
    output.push('\n');
    output.push_str("locality-machineid = ");
    output.push_str(service.machine_id.as_str());
    output.push('\n');
    output.push_str("locality-zoneid = ");
    output.push_str(service.zone_id.as_str());
    output.push('\n');
    output.push_str("locality-dcid = ");
    output.push_str(service.datacenter_id.as_str());
    output.push('\n');
    output.push_str("class = ");
    output.push_str(service.process_class.as_str());
    output.push('\n');
    output.push_str("parentpid = $PID");
    output.push_str("\n\n[fdbserver.");
    output.push_str(service.listen_port.to_string().as_str());
    output.push_str("]\n");
    output
}

async fn configure_new_database(
    service: &FoundationDbServiceSpec,
    command_timeout: Duration,
) -> Result<(), FoundationDbServiceError> {
    let mut command_text = String::from("configure new ");
    command_text.push_str(service.redundancy_mode.as_str());
    command_text.push(' ');
    command_text.push_str(service.storage_engine.as_str());

    let mut remaining = FOUNDATIONDB_CONFIGURE_RETRY_ATTEMPTS;
    let mut last_failure = FoundationDbServiceError::ConfigureNewFailed;
    while remaining > 0 {
        let configure_failure = match run_fixed_command(
            FDBCLI_BINARY,
            [
                "-C",
                FOUNDATIONDB_CLUSTER_FILE,
                "--no-status",
                "--exec",
                command_text.as_str(),
            ],
            command_timeout,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) => foundationdb_configure_error(error),
        };
        match run_fixed_command(
            FDBCLI_BINARY,
            ["-C", FOUNDATIONDB_CLUSTER_FILE, "--exec", "status minimal"],
            command_timeout,
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_failure = preferred_foundationdb_retry_error(
                    configure_failure,
                    foundationdb_status_error(error),
                );
            }
        }
        remaining = remaining
            .checked_sub(1)
            .ok_or(FoundationDbServiceError::LocalCommand)?;
        if remaining > 0 {
            sleep(FOUNDATIONDB_STATUS_RETRY_DELAY).await;
        }
    }
    Err(last_failure)
}

async fn configure_existing_database(
    service: &FoundationDbServiceSpec,
    command_timeout: Duration,
) -> Result<(), FoundationDbServiceError> {
    let mut command_text = String::from("configure ");
    command_text.push_str(service.redundancy_mode.as_str());
    command_text.push(' ');
    command_text.push_str(service.storage_engine.as_str());

    match run_fixed_command(
        FDBCLI_BINARY,
        [
            "-C",
            FOUNDATIONDB_CLUSTER_FILE,
            "--no-status",
            "--exec",
            command_text.as_str(),
        ],
        command_timeout,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(error) => Err(foundationdb_configure_error(error)),
    }
}

async fn wait_for_foundationdb_convergence(
    service: &FoundationDbServiceSpec,
    command_timeout: Duration,
    attempts: usize,
    retry_delay: Duration,
) -> Result<(), FoundationDbServiceError> {
    let mut remaining = attempts;
    let mut last_failure = FoundationDbServiceError::StatusUnavailable;
    while remaining > 0 {
        match fetch_foundationdb_status_observation(command_timeout).await {
            Ok(observation) => match validate_foundationdb_observation(service, &observation) {
                Ok(()) => return Ok(()),
                Err(error) => last_failure = error,
            },
            Err(error) => last_failure = foundationdb_status_error(error),
        }
        remaining = remaining
            .checked_sub(1)
            .ok_or(FoundationDbServiceError::LocalCommand)?;
        if remaining > 0 {
            sleep(retry_delay).await;
        }
    }
    Err(last_failure)
}

async fn fetch_foundationdb_status_observation(
    command_timeout: Duration,
) -> Result<FoundationDbStatusObservation, FoundationDbServiceError> {
    let stdout = run_fixed_command_output(
        FDBCLI_BINARY,
        ["-C", FOUNDATIONDB_CLUSTER_FILE, "--exec", "status json"],
        command_timeout,
    )
    .await?;
    FoundationDbStatusObservation::from_json_bytes(stdout.as_slice())
}

fn validate_foundationdb_observation(
    service: &FoundationDbServiceSpec,
    observation: &FoundationDbStatusObservation,
) -> Result<(), FoundationDbServiceError> {
    if let Some(false) = observation.coordinator_quorum_reachable {
        return Err(FoundationDbServiceError::CoordinatorQuorumLost);
    }
    if observation
        .coordinator_addresses
        .as_ref()
        .is_some_and(|addresses| !coordinator_sets_match(service, addresses))
    {
        return Err(FoundationDbServiceError::StaleClusterFile);
    }
    if observation.database_available != Some(true) {
        return Err(FoundationDbServiceError::DatabaseUnavailable);
    }
    if !observation.local_process_recruited(service.public_address.as_str(), service.listen_port) {
        return Err(FoundationDbServiceError::RecruitmentPending);
    }
    if !observed_storage_supports_redundancy(
        observation.storage_process_count,
        service.redundancy_mode,
    ) {
        return Err(FoundationDbServiceError::InsufficientStorageForRedundancy);
    }
    if observation
        .recovery_state
        .as_deref()
        .is_some_and(|state| state != "fully_recovered")
    {
        return Err(FoundationDbServiceError::RecoveryInProgress);
    }
    if observation.data_movement_in_progress {
        return Err(FoundationDbServiceError::DataMovementInProgress);
    }
    Ok(())
}

fn coordinator_sets_match(service: &FoundationDbServiceSpec, observed: &[String]) -> bool {
    if observed.len() != service.coordinators.len() {
        return false;
    }
    service.coordinators.iter().all(|coordinator| {
        let mut address = coordinator.address.clone();
        address.push(':');
        address.push_str(coordinator.port.to_string().as_str());
        observed.iter().any(|observed_address| {
            observed_address == address.as_str() || observed_address.starts_with(address.as_str())
        })
    })
}

fn observed_storage_supports_redundancy(
    storage_process_count: usize,
    redundancy_mode: FoundationDbRedundancyMode,
) -> bool {
    match redundancy_mode {
        FoundationDbRedundancyMode::Single => storage_process_count >= 1,
        FoundationDbRedundancyMode::Double => storage_process_count >= 2,
        FoundationDbRedundancyMode::Triple => storage_process_count >= 5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FoundationDbStatusObservation {
    database_available: Option<bool>,
    coordinator_quorum_reachable: Option<bool>,
    coordinator_addresses: Option<Vec<String>>,
    process_addresses: Vec<String>,
    storage_process_count: usize,
    recovery_state: Option<String>,
    data_movement_in_progress: bool,
}

impl FoundationDbStatusObservation {
    fn from_json_bytes(bytes: &[u8]) -> Result<Self, FoundationDbServiceError> {
        let value = serde_json::from_slice::<Value>(bytes)
            .map_err(|_error| FoundationDbServiceError::StatusUnavailable)?;
        Ok(Self {
            database_available: value_at_path(&value, &["client", "database_status", "available"])
                .and_then(Value::as_bool),
            coordinator_quorum_reachable: value_at_path(
                &value,
                &["client", "coordinators", "quorum_reachable"],
            )
            .and_then(Value::as_bool),
            coordinator_addresses: coordinator_addresses_from_status(&value),
            process_addresses: process_addresses_from_status(&value),
            storage_process_count: storage_process_count_from_status(&value),
            recovery_state: value_at_path(&value, &["cluster", "recovery_state", "name"])
                .and_then(Value::as_str)
                .map(str::to_owned),
            data_movement_in_progress: data_movement_in_progress(&value),
        })
    }

    fn local_process_recruited(&self, public_address: &str, listen_port: u16) -> bool {
        let mut expected = public_address.to_owned();
        expected.push(':');
        expected.push_str(listen_port.to_string().as_str());
        self.process_addresses
            .iter()
            .any(|address| address == expected.as_str() || address.starts_with(expected.as_str()))
    }
}

async fn run_fixed_command<const N: usize>(
    binary: &str,
    args: [&str; N],
    command_timeout: Duration,
) -> Result<(), FoundationDbServiceError> {
    run_fixed_command_output(binary, args, command_timeout)
        .await
        .map(|_stdout| ())
}

async fn run_fixed_command_output<const N: usize>(
    binary: &str,
    args: [&str; N],
    command_timeout: Duration,
) -> Result<Vec<u8>, FoundationDbServiceError> {
    let mut command = Command::new(binary);
    command.kill_on_drop(true);
    command.args(args);
    let result = timeout(command_timeout, command.output()).await;
    match result {
        Ok(Ok(output)) if output.status.success() => Ok(output.stdout),
        Ok(Ok(output)) => Err(classify_foundationdb_command_output(
            output.stdout.as_slice(),
            output.stderr.as_slice(),
        )),
        Ok(Err(_)) | Err(_) => Err(FoundationDbServiceError::LocalCommand),
    }
}

fn foundationdb_command_timeout(configured_timeout: Duration) -> Duration {
    configured_timeout.min(FOUNDATIONDB_LOCAL_COMMAND_TIMEOUT)
}

fn foundationdb_configure_error(error: FoundationDbServiceError) -> FoundationDbServiceError {
    match error {
        FoundationDbServiceError::ConfigurationInvalid
        | FoundationDbServiceError::CoordinatorsUnavailable
        | FoundationDbServiceError::CoordinatorQuorumLost
        | FoundationDbServiceError::DatabaseUnavailable => error,
        FoundationDbServiceError::DatabaseNotConfigured => {
            FoundationDbServiceError::ConfigureNewFailed
        }
        _ => FoundationDbServiceError::ConfigureNewFailed,
    }
}

fn foundationdb_status_error(error: FoundationDbServiceError) -> FoundationDbServiceError {
    match error {
        FoundationDbServiceError::CoordinatorsUnavailable
        | FoundationDbServiceError::CoordinatorQuorumLost
        | FoundationDbServiceError::DatabaseNotConfigured
        | FoundationDbServiceError::DatabaseUnavailable => error,
        _ => FoundationDbServiceError::StatusUnavailable,
    }
}

fn preferred_foundationdb_retry_error(
    configure_failure: FoundationDbServiceError,
    status_failure: FoundationDbServiceError,
) -> FoundationDbServiceError {
    match status_failure {
        FoundationDbServiceError::StatusUnavailable => configure_failure,
        _ => status_failure,
    }
}

fn classify_foundationdb_command_output(stdout: &[u8], stderr: &[u8]) -> FoundationDbServiceError {
    if contains_ascii_case_insensitive(stdout, b"coordination servers")
        || contains_ascii_case_insensitive(stderr, b"coordination servers")
        || contains_ascii_case_insensitive(stdout, b"coordinators")
        || contains_ascii_case_insensitive(stderr, b"coordinators")
    {
        return FoundationDbServiceError::CoordinatorsUnavailable;
    }
    if contains_ascii_case_insensitive(stdout, b"quorum")
        || contains_ascii_case_insensitive(stderr, b"quorum")
    {
        return FoundationDbServiceError::CoordinatorQuorumLost;
    }
    if contains_ascii_case_insensitive(stdout, b"database is not configured")
        || contains_ascii_case_insensitive(stderr, b"database is not configured")
        || contains_ascii_case_insensitive(stdout, b"database is not yet configured")
        || contains_ascii_case_insensitive(stderr, b"database is not yet configured")
        || contains_ascii_case_insensitive(stdout, b"has not been configured")
        || contains_ascii_case_insensitive(stderr, b"has not been configured")
    {
        return FoundationDbServiceError::DatabaseNotConfigured;
    }
    if contains_ascii_case_insensitive(stdout, b"database is unavailable")
        || contains_ascii_case_insensitive(stderr, b"database is unavailable")
    {
        return FoundationDbServiceError::DatabaseUnavailable;
    }
    if contains_ascii_case_insensitive(stdout, b"invalid configuration")
        || contains_ascii_case_insensitive(stderr, b"invalid configuration")
        || contains_ascii_case_insensitive(stdout, b"unknown option")
        || contains_ascii_case_insensitive(stderr, b"unknown option")
    {
        return FoundationDbServiceError::ConfigurationInvalid;
    }
    FoundationDbServiceError::LocalCommand
}

fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn coordinator_addresses_from_status(value: &Value) -> Option<Vec<String>> {
    let coordinators = value_at_path(value, &["client", "coordinators", "coordinators"])?;
    let array = coordinators.as_array()?;
    let addresses = array
        .iter()
        .filter_map(coordinator_address)
        .collect::<Vec<_>>();
    if addresses.is_empty() {
        None
    } else {
        Some(addresses)
    }
}

fn coordinator_address(value: &Value) -> Option<String> {
    value
        .get("address")
        .and_then(Value::as_str)
        .or_else(|| value.get("reachable").and_then(Value::as_str))
        .map(str::to_owned)
}

fn process_addresses_from_status(value: &Value) -> Vec<String> {
    let Some(processes) =
        value_at_path(value, &["cluster", "processes"]).and_then(Value::as_object)
    else {
        return Vec::new();
    };
    processes
        .values()
        .filter_map(|process| {
            process
                .get("address")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn storage_process_count_from_status(value: &Value) -> usize {
    let Some(processes) =
        value_at_path(value, &["cluster", "processes"]).and_then(Value::as_object)
    else {
        return 0;
    };
    processes
        .values()
        .filter(|process| process_has_storage_role(process))
        .count()
}

fn process_has_storage_role(process: &Value) -> bool {
    process
        .get("class_type")
        .and_then(Value::as_str)
        .is_some_and(|class_type| class_type == "storage")
        || process
            .get("roles")
            .and_then(Value::as_array)
            .is_some_and(|roles| roles.iter().any(role_is_storage))
}

fn role_is_storage(role: &Value) -> bool {
    role.get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| role == "storage")
}

fn data_movement_in_progress(value: &Value) -> bool {
    let Some(moving_data) = value_at_path(value, &["cluster", "data", "moving_data"]) else {
        return false;
    };
    positive_u64_at_path(moving_data, &["in_flight_bytes"])
        || positive_u64_at_path(moving_data, &["in_queue_bytes"])
        || positive_u64_at_path(moving_data, &["highest_priority"])
        || moving_data
            .get("state")
            .and_then(Value::as_str)
            .is_some_and(|state| state != "healthy")
}

fn positive_u64_at_path(value: &Value, path: &[&str]) -> bool {
    value_at_path(value, path)
        .and_then(Value::as_u64)
        .is_some_and(|number| number > 0)
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| ascii_bytes_equal_ignore_case(window, needle))
}

fn ascii_bytes_equal_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right.iter())
            .all(|(left_byte, right_byte)| left_byte.eq_ignore_ascii_case(right_byte))
}

fn validate_ip_address(value: String) -> Result<String, FoundationDbServiceError> {
    value
        .parse::<IpAddr>()
        .map_err(|_error| FoundationDbServiceError::InvalidAddress)?;
    Ok(value)
}

fn validate_port(value: u32) -> Result<u16, FoundationDbServiceError> {
    if !(MIN_PORT..=MAX_PORT).contains(&value) {
        return Err(FoundationDbServiceError::InvalidAddress);
    }
    u16::try_from(value).map_err(|_error| FoundationDbServiceError::InvalidAddress)
}

fn validate_cluster_token(value: String) -> Result<String, FoundationDbServiceError> {
    if value.is_empty()
        || value.len() > MAX_TOKEN_LEN
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(FoundationDbServiceError::InvalidIdentifier);
    }
    Ok(value)
}

fn validate_locality_id(value: String) -> Result<String, FoundationDbServiceError> {
    if value.is_empty()
        || value.len() > MAX_ID_LEN
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(FoundationDbServiceError::InvalidIdentifier);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{
        FOUNDATIONDB_LOCAL_COMMAND_TIMEOUT, FoundationDbClusterOperation, FoundationDbCoordinator,
        FoundationDbProcessClass, FoundationDbRedundancyMode, FoundationDbServiceError,
        FoundationDbServiceSpec, FoundationDbStatusObservation, FoundationDbStorageEngine,
        classify_foundationdb_command_output, foundationdb_command_timeout,
        validate_foundationdb_observation,
    };
    use std::time::Duration;

    #[test]
    fn foundationdb_command_timeout_caps_global_command_timeout() {
        let configured_timeout = FOUNDATIONDB_LOCAL_COMMAND_TIMEOUT
            .checked_mul(4)
            .expect("test timeout multiplication should fit");

        assert_eq!(
            foundationdb_command_timeout(configured_timeout),
            FOUNDATIONDB_LOCAL_COMMAND_TIMEOUT
        );
    }

    #[test]
    fn foundationdb_command_timeout_preserves_smaller_configured_timeout() {
        let configured_timeout = Duration::from_secs(1);

        assert_eq!(
            foundationdb_command_timeout(configured_timeout),
            configured_timeout
        );
    }

    #[tokio::test]
    async fn fixed_command_timeout_returns_local_command_error() {
        let result = super::run_fixed_command("/bin/sleep", ["5"], Duration::from_millis(10)).await;

        assert_eq!(result, Err(FoundationDbServiceError::LocalCommand));
    }

    #[test]
    fn classifies_foundationdb_coordination_server_diagnostic() {
        let error = classify_foundationdb_command_output(
            b"The database is unavailable.",
            b"Unable to reach coordination servers.",
        );

        assert_eq!(error, FoundationDbServiceError::CoordinatorsUnavailable);
    }

    #[test]
    fn classifies_foundationdb_not_configured_diagnostic() {
        let error =
            classify_foundationdb_command_output(b"The database is not yet configured.", b"");

        assert_eq!(error, FoundationDbServiceError::DatabaseNotConfigured);
    }

    #[test]
    fn classifies_foundationdb_unavailable_diagnostic() {
        let error = classify_foundationdb_command_output(b"The database is unavailable.", b"");

        assert_eq!(error, FoundationDbServiceError::DatabaseUnavailable);
    }

    #[test]
    fn classifies_foundationdb_invalid_configuration_diagnostic() {
        let error = classify_foundationdb_command_output(b"ERROR: Invalid configuration.", b"");

        assert_eq!(error, FoundationDbServiceError::ConfigurationInvalid);
    }

    #[test]
    fn parses_status_json_into_typed_observation() {
        let observation = FoundationDbStatusObservation::from_json_bytes(
            status_json("fully_recovered", 0, true).as_bytes(),
        )
        .expect("status json should parse");

        assert_eq!(observation.database_available, Some(true));
        assert_eq!(observation.coordinator_quorum_reachable, Some(true));
        assert_eq!(observation.storage_process_count, 2);
        assert!(observation.local_process_recruited("100.83.14.29", 4500));
        assert!(!observation.data_movement_in_progress);
    }

    #[test]
    fn validates_converged_status_for_add_node() {
        let observation = FoundationDbStatusObservation::from_json_bytes(
            status_json("fully_recovered", 0, true).as_bytes(),
        )
        .expect("status json should parse");

        assert_eq!(
            validate_foundationdb_observation(
                &service_spec(FoundationDbRedundancyMode::Double),
                &observation
            ),
            Ok(())
        );
    }

    #[test]
    fn detects_unrecruited_local_process_from_status_json() {
        let mut service = service_spec(FoundationDbRedundancyMode::Double);
        service.public_address = "100.83.14.99".to_owned();
        let observation = FoundationDbStatusObservation::from_json_bytes(
            status_json("fully_recovered", 0, true).as_bytes(),
        )
        .expect("status json should parse");

        assert_eq!(
            validate_foundationdb_observation(&service, &observation),
            Err(FoundationDbServiceError::RecruitmentPending)
        );
    }

    #[test]
    fn detects_recovery_and_data_movement_from_status_json() {
        let recovery = FoundationDbStatusObservation::from_json_bytes(
            status_json("recruiting_transaction_servers", 0, true).as_bytes(),
        )
        .expect("status json should parse");
        let moving = FoundationDbStatusObservation::from_json_bytes(
            status_json("fully_recovered", 1024, true).as_bytes(),
        )
        .expect("status json should parse");

        assert_eq!(
            validate_foundationdb_observation(
                &service_spec(FoundationDbRedundancyMode::Double),
                &recovery
            ),
            Err(FoundationDbServiceError::RecoveryInProgress)
        );
        assert_eq!(
            validate_foundationdb_observation(
                &service_spec(FoundationDbRedundancyMode::Double),
                &moving
            ),
            Err(FoundationDbServiceError::DataMovementInProgress)
        );
    }

    #[test]
    fn detects_stale_coordinator_set_and_insufficient_storage() {
        let stale = FoundationDbStatusObservation::from_json_bytes(
            status_json_with_coordinator("100.83.14.31:4500", "fully_recovered", 0, true)
                .as_bytes(),
        )
        .expect("status json should parse");
        let insufficient = FoundationDbStatusObservation::from_json_bytes(
            status_json("fully_recovered", 0, true).as_bytes(),
        )
        .expect("status json should parse");

        assert_eq!(
            validate_foundationdb_observation(
                &service_spec(FoundationDbRedundancyMode::Double),
                &stale
            ),
            Err(FoundationDbServiceError::StaleClusterFile)
        );
        assert_eq!(
            validate_foundationdb_observation(
                &service_spec(FoundationDbRedundancyMode::Triple),
                &insufficient
            ),
            Err(FoundationDbServiceError::InsufficientStorageForRedundancy)
        );
    }

    #[test]
    fn detects_coordinator_quorum_loss_from_status_json() {
        let observation = FoundationDbStatusObservation::from_json_bytes(
            status_json("fully_recovered", 0, false).as_bytes(),
        )
        .expect("status json should parse");

        assert_eq!(
            validate_foundationdb_observation(
                &service_spec(FoundationDbRedundancyMode::Double),
                &observation
            ),
            Err(FoundationDbServiceError::CoordinatorQuorumLost)
        );
    }

    fn service_spec(redundancy_mode: FoundationDbRedundancyMode) -> FoundationDbServiceSpec {
        FoundationDbServiceSpec {
            cluster_description: "fdb_staging_hel".to_owned(),
            cluster_token: "0123456789abcdef0123456789abcdef".to_owned(),
            coordinators: vec![FoundationDbCoordinator {
                address: "100.83.14.29".to_owned(),
                port: 4500,
            }],
            public_address: "100.83.14.29".to_owned(),
            listen_port: 4500,
            datacenter_id: "hel".to_owned(),
            zone_id: "hel-01".to_owned(),
            machine_id: "hetzner-staging-fdb-hel-01".to_owned(),
            process_class: FoundationDbProcessClass::Storage,
            redundancy_mode,
            storage_engine: FoundationDbStorageEngine::Ssd2,
            initialize_database: false,
            cluster_operation: FoundationDbClusterOperation::AddNode,
        }
    }

    fn status_json(
        recovery_state: &str,
        moving_bytes: u64,
        coordinator_quorum_reachable: bool,
    ) -> String {
        status_json_with_coordinator(
            "100.83.14.29:4500",
            recovery_state,
            moving_bytes,
            coordinator_quorum_reachable,
        )
    }

    fn status_json_with_coordinator(
        coordinator_address: &str,
        recovery_state: &str,
        moving_bytes: u64,
        coordinator_quorum_reachable: bool,
    ) -> String {
        serde_json::json!({
            "client": {
                "database_status": {
                    "available": true
                },
                "coordinators": {
                    "quorum_reachable": coordinator_quorum_reachable,
                    "coordinators": [
                        {
                            "address": coordinator_address
                        }
                    ]
                }
            },
            "cluster": {
                "recovery_state": {
                    "name": recovery_state
                },
                "data": {
                    "moving_data": {
                        "in_flight_bytes": moving_bytes,
                        "in_queue_bytes": 0
                    }
                },
                "processes": {
                    "process-1": {
                        "address": "100.83.14.29:4500",
                        "class_type": "storage",
                        "roles": [
                            {
                                "role": "storage"
                            }
                        ]
                    },
                    "process-2": {
                        "address": "100.83.14.30:4500",
                        "class_type": "storage",
                        "roles": [
                            {
                                "role": "storage"
                            }
                        ]
                    }
                }
            }
        })
        .to_string()
    }
}
