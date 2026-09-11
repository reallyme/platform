// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Agent runtime loop.

use std::{future::Future, mem, path::Path, time::Duration};

use reallyme_hephaestus_contract::agent_registration_signature_payload;
use reallyme_hephaestus_contract::generated::proto::reallyme::domain::v1 as domain_pb;
use reallyme_hephaestus_contract::generated::proto::reallyme::hephaestus::v1 as hephaestus_pb;
use reallyme_hephaestus_domain::{HephaestusAgentBootSecret, HephaestusAgentRuntimeToken};
use secrecy::ExposeSecret;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use zeroize::Zeroize;

use crate::actions::execute_polled_actions;
use crate::config::HephaestusAgentConfig;
use crate::control_plane::HephaestusControlPlaneClient;
use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::identity::{AgentIdentity, sign_registration_payload};
use crate::report::{build_agent_report, build_boot_report};
use crate::state::{AgentTokenStore, MAX_RUNTIME_TOKEN_BYTES, MIN_RUNTIME_TOKEN_BYTES};

const AGENT_VERSION: &str = concat!("hephaestus-agent/", env!("CARGO_PKG_VERSION"));
const FIRST_REGISTRATION_INITIAL_RETRY_DELAY: Duration = Duration::from_secs(2);
const FIRST_REGISTRATION_MAX_RETRY_DELAY: Duration = Duration::from_secs(60);
const ACTION_EXECUTION_RESULT_QUEUE_CAPACITY: usize = 1;
/// Runs the agent until shutdown.
pub async fn run(config_path: &Path) -> AgentResult<()> {
    let config = HephaestusAgentConfig::load(config_path).await?;
    ensure_state_dir(config.state_dir()).await?;
    let token_store = AgentTokenStore::new(config.state_dir(), config.bootstrap_token_path())?;
    let registration = ensure_registered(&config, &token_store).await?;
    tracing::info!(
        desired_generation = registration.desired_generation,
        "hephaestus agent registration ready"
    );
    report_loop(
        &config,
        token_store,
        registration.client,
        registration.desired_generation,
    )
    .await
}

async fn ensure_state_dir(path: &Path) -> AgentResult<()> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

struct AgentRegistration {
    client: HephaestusControlPlaneClient,
    desired_generation: u64,
}

async fn ensure_registered(
    config: &HephaestusAgentConfig,
    token_store: &AgentTokenStore,
) -> AgentResult<AgentRegistration> {
    if let Some(runtime_token) = token_store.read_runtime_token().await? {
        let observed_generation = token_store.read_observed_generation().await?.unwrap_or(0);
        tracing::info!(
            observed_generation,
            "loaded persisted hephaestus agent runtime token"
        );
        return Ok(AgentRegistration {
            client: HephaestusControlPlaneClient::for_runtime(config, &runtime_token)?,
            desired_generation: observed_generation,
        });
    }

    let bootstrap_token = match bootstrap_token(config, token_store).await {
        Ok(token) => token,
        Err(error) if error.reason() == HephaestusAgentErrorReason::BootstrapTokenUnavailable => {
            tracing::error!(
                reason = ?error.reason(),
                "agent runtime token is unavailable and no bootstrap token is present; central re-enrollment must reissue bootstrap material"
            );
            return Err(error);
        }
        Err(error) => return Err(error),
    };
    let signing_key = token_store.load_or_create_identity_key().await?;
    let identity = AgentIdentity::from_signing_key(&signing_key)?;
    let registration_client = HephaestusControlPlaneClient::for_registration(config)?;
    let report = build_boot_report(config).await?;
    let registration_payload = agent_registration_signature_payload(
        &report,
        AGENT_VERSION,
        identity.public_key(),
        identity.public_key_fingerprint(),
    );
    let registration_signature =
        sign_registration_payload(&signing_key, registration_payload.as_str())?;
    let mut response = register_agent_with_retry(
        || {
            registration_client.register_agent(
                &report,
                &bootstrap_token,
                AGENT_VERSION,
                identity.public_key(),
                identity.public_key_fingerprint(),
                registration_signature.as_str(),
            )
        },
        FIRST_REGISTRATION_INITIAL_RETRY_DELAY,
        FIRST_REGISTRATION_MAX_RETRY_DELAY,
        tokio::time::sleep,
    )
    .await?;
    let result = response.result.as_known().ok_or_else(|| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::RegistrationRejected)
    })?;
    if result
        != domain_pb::HephaestusAgentBootReportResult::HEPHAESTUS_AGENT_BOOT_REPORT_RESULT_ACCEPTED
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::RegistrationRejected,
        ));
    }
    let runtime_token =
        runtime_token_from_registration_response_owned(mem::take(&mut response.runtime_token))?;

    token_store.write_runtime_token(&runtime_token).await?;
    let client = HephaestusControlPlaneClient::for_runtime(config, &runtime_token)?;
    token_store
        .write_observed_generation(response.desired_generation)
        .await?;
    token_store.delete_bootstrap_token().await?;
    tracing::info!(
        desired_generation = response.desired_generation,
        "hephaestus agent first boot registration accepted"
    );

    Ok(AgentRegistration {
        client,
        desired_generation: response.desired_generation,
    })
}

async fn register_agent_with_retry<F, Fut, S, SleepFut>(
    mut register: F,
    initial_retry_delay: Duration,
    max_retry_delay: Duration,
    mut sleep: S,
) -> AgentResult<hephaestus_pb::RegisterAgentResponse>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = AgentResult<hephaestus_pb::RegisterAgentResponse>>,
    S: FnMut(Duration) -> SleepFut,
    SleepFut: Future<Output = ()>,
{
    let mut retry_delay = initial_retry_delay;
    let mut attempt = 1u64;
    loop {
        match register().await {
            Ok(response) => return Ok(response),
            Err(error) if error.reason() == HephaestusAgentErrorReason::ConnectFailed => {
                tracing::warn!(
                    attempt,
                    retry_delay_ms = retry_delay.as_millis(),
                    reason = ?error.reason(),
                    "first boot agent registration failed transiently; retrying without exiting"
                );
                sleep(retry_delay).await;
                retry_delay = next_registration_retry_delay(retry_delay, max_retry_delay);
                attempt = attempt.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }
}

fn next_registration_retry_delay(current: Duration, max_retry_delay: Duration) -> Duration {
    current
        .checked_mul(2)
        .unwrap_or(max_retry_delay)
        .min(max_retry_delay)
}

fn runtime_token_from_registration_response(
    runtime_token: &str,
) -> AgentResult<HephaestusAgentRuntimeToken> {
    if runtime_token.len() < MIN_RUNTIME_TOKEN_BYTES
        || runtime_token.len() > MAX_RUNTIME_TOKEN_BYTES
    {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::RuntimeTokenUnavailable,
        ));
    }
    HephaestusAgentRuntimeToken::new(runtime_token).map_err(|_error| {
        HephaestusAgentError::new(HephaestusAgentErrorReason::RuntimeTokenUnavailable)
    })
}

fn runtime_token_from_registration_response_owned(
    mut runtime_token: String,
) -> AgentResult<HephaestusAgentRuntimeToken> {
    let result = runtime_token_from_registration_response(runtime_token.as_str());
    runtime_token.zeroize();
    result
}

async fn bootstrap_token(
    config: &HephaestusAgentConfig,
    token_store: &AgentTokenStore,
) -> AgentResult<HephaestusAgentBootSecret> {
    if let Some(token) = config.inline_bootstrap_token() {
        return HephaestusAgentBootSecret::new(token.expose_secret()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::BootstrapTokenUnavailable)
        });
    }
    token_store.read_bootstrap_token().await
}

#[cfg(test)]
mod tests;

async fn report_loop(
    config: &HephaestusAgentConfig,
    token_store: AgentTokenStore,
    client: HephaestusControlPlaneClient,
    initial_desired_generation: u64,
) -> AgentResult<()> {
    let (action_result_tx, mut action_result_rx) =
        mpsc::channel::<ActionExecutionResult>(ACTION_EXECUTION_RESULT_QUEUE_CAPACITY);
    let mut full_report_interval = tokio::time::interval(config.full_report_interval());
    full_report_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut action_poll_interval = tokio::time::interval(config.action_poll_interval());
    action_poll_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut observed_generation = initial_desired_generation;
    let mut last_persisted_generation = initial_desired_generation;
    let mut action_execution_in_flight = false;
    tracing::info!(
        observed_generation,
        full_report_interval_ms = config.full_report_interval().as_millis(),
        action_poll_interval_ms = config.action_poll_interval().as_millis(),
        "hephaestus agent runtime loop started"
    );
    loop {
        tokio::select! {
            _ = full_report_interval.tick() => {
                submit_agent_report_or_continue(config, &client).await?;
            }
            Some(action_result) = action_result_rx.recv() => {
                action_execution_in_flight = false;
                let action_generation = match action_result.result {
                    Ok(generation) => generation,
                    Err(error) => return Err(error),
                };
                observed_generation = observed_generation.max(action_generation);
                if observed_generation != last_persisted_generation {
                    token_store.write_observed_generation(observed_generation).await?;
                    last_persisted_generation = observed_generation;
                }
                tracing::debug!(
                    observed_generation,
                    "completed hephaestus agent action execution"
                );
                if action_result.action_count > 0 {
                    submit_agent_report_or_continue(config, &client).await?;
                }
            }
            _ = action_poll_interval.tick() => {
                if action_execution_in_flight {
                    tracing::debug!(
                        observed_generation,
                        "skipping action poll while prior action batch is still executing"
                    );
                    continue;
                }
                let response = match client
                    .poll_agent_actions(config.server_id(), AGENT_VERSION, observed_generation)
                    .await
                {
                    Ok(response) => response,
                    Err(error) if error.reason() == HephaestusAgentErrorReason::ConnectFailed => {
                        tracing::warn!(
                            reason = ?error.reason(),
                            "failed to poll agent actions; will retry on next interval"
                        );
                        continue;
                    }
                    Err(error) => return Err(error),
                };
                let action_count = response.actions.len();
                tracing::info!(
                    action_count,
                    observed_generation,
                    "polled hephaestus agent actions"
                );
                if action_count > 0 {
                    action_execution_in_flight = true;
                    spawn_action_execution(
                        config.clone(),
                        client.clone(),
                        response.actions,
                        action_result_tx.clone(),
                    );
                }
            }
            signal = tokio::signal::ctrl_c() => {
                signal.map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::CommandFailed))?;
                return Ok(());
            }
        }
    }
}

async fn submit_agent_report_or_continue(
    config: &HephaestusAgentConfig,
    client: &HephaestusControlPlaneClient,
) -> AgentResult<()> {
    let report = match build_agent_report(config).await {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(
                reason = ?error.reason(),
                "failed to build agent report; will retry on next interval"
            );
            return Ok(());
        }
    };
    if let Err(error) = client.submit_agent_report(&report).await {
        tracing::warn!(
            reason = ?error.reason(),
            "failed to submit agent report; will retry on next interval"
        );
        return Ok(());
    }
    tracing::debug!("submitted hephaestus agent report");
    Ok(())
}

struct ActionExecutionResult {
    action_count: usize,
    result: AgentResult<u64>,
}

fn spawn_action_execution(
    config: HephaestusAgentConfig,
    client: HephaestusControlPlaneClient,
    actions: Vec<hephaestus_pb::AgentAction>,
    action_result_tx: mpsc::Sender<ActionExecutionResult>,
) {
    let action_count = actions.len();
    // The node-local agent is intentionally independent of the server-kit task
    // supervisor. We still isolate action execution in one bounded worker so
    // health reporting cannot be starved by long local maintenance commands.
    tokio::spawn(async move {
        let result = match execute_polled_actions(&config, &client, actions).await {
            Ok(generation) => Ok(generation),
            Err(error) => {
                if action_count > 0 {
                    let _report_result = submit_agent_report_or_continue(&config, &client).await;
                }
                Err(error)
            }
        };
        let _send_result = action_result_tx
            .send(ActionExecutionResult {
                action_count,
                result,
            })
            .await;
    });
}
