// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Local durable state for the node agent.

use std::path::{Path, PathBuf};

use reallyme_hephaestus_domain::{HephaestusAgentBootSecret, HephaestusAgentRuntimeToken};
use secrecy::{ExposeSecret, SecretString};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use zeroize::Zeroizing;

use crate::error::{AgentResult, HephaestusAgentError, HephaestusAgentErrorReason};
use crate::identity::{
    AgentSigningKey, decode_signing_key, encode_signing_key, generate_signing_key,
};

const RUNTIME_TOKEN_FILE_NAME: &str = "runtime-token";
const OBSERVED_GENERATION_FILE_NAME: &str = "observed-generation";
const IDENTITY_PRIVATE_KEY_FILE_NAME: &str = "ed25519-private-key";
const IDENTITY_PRIVATE_KEY_TMP_STEM: &str = "ed25519-private-key.";
const IDENTITY_PRIVATE_KEY_TMP_EXTENSION: &str = ".tmp";
pub(crate) const MAX_RUNTIME_TOKEN_BYTES: usize = 4 * 1024;
pub(crate) const MIN_RUNTIME_TOKEN_BYTES: usize = 16;
// Desired generations are server-issued monotonic values. Hephaestus currently
// uses Unix seconds for generated actions, so the cap must accept present-day
// epoch values while still rejecting obviously corrupt state.
const MAX_OBSERVED_GENERATION: u64 = 9_999_999_999;
const MAX_IDENTITY_KEY_TMP_NUMERIC_PART_BYTES: usize = 20;
const MAX_TOKEN_FILE_BYTES: u64 = 4096;

/// File-backed token state owned by the agent.
#[derive(Debug, Clone)]
pub struct AgentTokenStore {
    state_dir: PathBuf,
    bootstrap_token_path: PathBuf,
}

impl AgentTokenStore {
    /// Constructs a token store from validated absolute paths.
    pub fn new(state_dir: &Path, bootstrap_token_path: &Path) -> AgentResult<Self> {
        validate_absolute_path(state_dir)?;
        validate_absolute_path(bootstrap_token_path)?;
        Ok(Self {
            state_dir: state_dir.to_path_buf(),
            bootstrap_token_path: bootstrap_token_path.to_path_buf(),
        })
    }

    /// Reads the persisted runtime token, if registration has already happened.
    pub async fn read_runtime_token(&self) -> AgentResult<Option<HephaestusAgentRuntimeToken>> {
        let path = self.runtime_token_path();
        if tokio::fs::metadata(path.as_path()).await.is_err() {
            return Ok(None);
        }
        let token = read_secret_file(path.as_path()).await?;
        validate_runtime_token_length(token.expose_secret())?;
        HephaestusAgentRuntimeToken::new(token.expose_secret())
            .map(Some)
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::RuntimeTokenUnavailable)
            })
    }

    /// Reads the one-time bootstrap token from cloud-init state.
    pub async fn read_bootstrap_token(&self) -> AgentResult<HephaestusAgentBootSecret> {
        let token = read_secret_file(self.bootstrap_token_path.as_path()).await?;
        HephaestusAgentBootSecret::new(token.expose_secret()).map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::BootstrapTokenUnavailable)
        })
    }

    /// Persists the runtime token with restrictive permissions.
    pub async fn write_runtime_token(
        &self,
        token: &HephaestusAgentRuntimeToken,
    ) -> AgentResult<()> {
        validate_runtime_token_length(token.expose_as_str())?;
        tokio::fs::create_dir_all(self.state_dir.as_path())
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })?;
        atomic_write_secret(
            self.runtime_token_path().as_path(),
            RUNTIME_TOKEN_FILE_NAME,
            token.expose_as_str(),
        )
        .await
    }

    /// Reads the latest action generation fully observed by this local agent.
    pub async fn read_observed_generation(&self) -> AgentResult<Option<u64>> {
        let path = self.observed_generation_path();
        if tokio::fs::metadata(path.as_path()).await.is_err() {
            return Ok(None);
        }
        let value = read_secret_file(path.as_path()).await?;
        let parsed = value.expose_secret().parse::<u64>().map_err(|_error| {
            HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig)
        })?;
        if parsed > MAX_OBSERVED_GENERATION {
            return Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::InvalidConfig,
            ));
        }
        Ok(Some(parsed))
    }

    /// Persists the latest action generation fully observed by this local agent.
    pub async fn write_observed_generation(&self, observed_generation: u64) -> AgentResult<()> {
        tokio::fs::create_dir_all(self.state_dir.as_path())
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })?;
        let rendered_generation = observed_generation.to_string();
        atomic_write_secret(
            self.observed_generation_path().as_path(),
            OBSERVED_GENERATION_FILE_NAME,
            rendered_generation.as_str(),
        )
        .await
    }

    /// Deletes the one-time bootstrap token after successful registration.
    pub async fn delete_bootstrap_token(&self) -> AgentResult<()> {
        match tokio::fs::remove_file(self.bootstrap_token_path.as_path()).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_error) => Err(HephaestusAgentError::new(
                HephaestusAgentErrorReason::FileDeleteFailed,
            )),
        }
    }

    /// Loads the durable node identity key or creates it once on first live boot.
    pub async fn load_or_create_identity_key(&self) -> AgentResult<AgentSigningKey> {
        self.cleanup_orphaned_identity_key_tmp_files().await?;

        let path = self.identity_private_key_path();
        if tokio::fs::metadata(path.as_path()).await.is_ok() {
            let encoded = read_secret_file(path.as_path()).await?;
            return decode_signing_key(encoded.expose_secret());
        }
        let signing_key = generate_signing_key()?;
        tokio::fs::create_dir_all(self.state_dir.as_path())
            .await
            .map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed)
            })?;
        let encoded = Zeroizing::new(encode_signing_key(&signing_key));
        if create_identity_key_if_absent(path.as_path(), encoded.as_str()).await? {
            Ok(signing_key)
        } else {
            let encoded = read_secret_file(path.as_path()).await?;
            decode_signing_key(encoded.expose_secret())
        }
    }

    fn runtime_token_path(&self) -> PathBuf {
        self.state_dir.join(RUNTIME_TOKEN_FILE_NAME)
    }

    fn observed_generation_path(&self) -> PathBuf {
        self.state_dir.join(OBSERVED_GENERATION_FILE_NAME)
    }

    fn identity_private_key_path(&self) -> PathBuf {
        self.state_dir.join(IDENTITY_PRIVATE_KEY_FILE_NAME)
    }

    async fn cleanup_orphaned_identity_key_tmp_files(&self) -> AgentResult<()> {
        let mut entries = match tokio::fs::read_dir(self.state_dir.as_path()).await {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_error) => {
                return Err(HephaestusAgentError::new(
                    HephaestusAgentErrorReason::FileReadFailed,
                ));
            }
        };

        loop {
            let Some(entry) = entries.next_entry().await.map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed)
            })?
            else {
                return Ok(());
            };

            let file_type = entry.file_type().await.map_err(|_error| {
                HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed)
            })?;
            if !file_type.is_file() {
                continue;
            }

            if is_orphaned_identity_tmp_file(entry.path().as_path())? {
                remove_file_if_exists(entry.path().as_path()).await?;
            }
        }
    }
}

fn validate_runtime_token_length(value: &str) -> AgentResult<()> {
    if value.len() < MIN_RUNTIME_TOKEN_BYTES || value.len() > MAX_RUNTIME_TOKEN_BYTES {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::RuntimeTokenUnavailable,
        ));
    }
    Ok(())
}

async fn read_secret_file(path: &Path) -> AgentResult<SecretString> {
    // Opening a preexisting FIFO can block before handle metadata is available.
    // Retain the preflight check as well as validating the opened handle below.
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_TOKEN_FILE_BYTES {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    let metadata = file
        .metadata()
        .await
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_TOKEN_FILE_BYTES {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    // Metadata is not a read bound: the file can grow after it is inspected.
    let limit = MAX_TOKEN_FILE_BYTES
        .checked_add(1)
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig))?;
    let mut bytes = Zeroizing::new(Vec::new());
    file.take(limit)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileReadFailed))?;
    if bytes.len() > MAX_RUNTIME_TOKEN_BYTES {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidConfig))?;
    let mut text = Zeroizing::new(value.to_owned());
    trim_secret_file_line_endings(&mut text);
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidConfig,
        ));
    }
    Ok(SecretString::new(
        std::mem::take(&mut *text).into_boxed_str(),
    ))
}

fn trim_secret_file_line_endings(value: &mut String) {
    while value.ends_with('\n') || value.ends_with('\r') {
        value.pop();
    }
}

async fn atomic_write_secret(
    path: &Path,
    temporary_file_stem: &str,
    value: &str,
) -> AgentResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    validate_absolute_path(parent)?;
    let temporary_path = parent.join(format!("{temporary_file_stem}.tmp"));
    let payload = Zeroizing::new(format!("{value}\n"));
    remove_file_if_exists(temporary_path.as_path()).await?;
    create_secret_file_new(temporary_path.as_path(), payload.as_bytes()).await?;
    tokio::fs::rename(temporary_path.as_path(), path)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

async fn create_identity_key_if_absent(path: &Path, value: &str) -> AgentResult<bool> {
    let parent = path
        .parent()
        .ok_or_else(|| HephaestusAgentError::new(HephaestusAgentErrorReason::InvalidPath))?;
    validate_absolute_path(parent)?;
    let payload = Zeroizing::new(format!("{value}\n"));
    let temporary_path = prepare_identity_key_file(parent, payload.as_bytes()).await?;
    match tokio::fs::hard_link(&temporary_path, path).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::FileWriteFailed,
        )),
    }
}

async fn prepare_identity_key_file(
    parent: &Path,
    contents: &[u8],
) -> AgentResult<tempfile::TempPath> {
    let parent = parent.to_path_buf();
    // RAII owns current temporary files. The old PID/timestamp cleanup pattern
    // must never match a file another current enrollment is still publishing.
    let temporary = tokio::task::spawn_blocking(move || {
        tempfile::Builder::new()
            .prefix(".ed25519-private-key-")
            .tempfile_in(parent)
    })
    .await
    .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?
    .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    let (file, temporary_path) = temporary.into_parts();
    let mut file = tokio::fs::File::from_std(file);
    file.write_all(contents)
        .await
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    file.sync_all()
        .await
        .map_err(|_| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    Ok(temporary_path)
}

fn is_orphaned_identity_tmp_file(path: &Path) -> AgentResult<bool> {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return Ok(false);
    };

    if !file_name.starts_with(IDENTITY_PRIVATE_KEY_TMP_STEM) {
        return Ok(false);
    }

    if !file_name.ends_with(IDENTITY_PRIVATE_KEY_TMP_EXTENSION) {
        return Ok(false);
    }

    let Some(without_extension) = file_name.strip_suffix(IDENTITY_PRIVATE_KEY_TMP_EXTENSION) else {
        return Ok(false);
    };
    let Some(middle) = without_extension.strip_prefix(IDENTITY_PRIVATE_KEY_TMP_STEM) else {
        return Ok(false);
    };
    let mut parts = middle.split('.');
    let pid = parts.next().unwrap_or("");
    let timestamp = parts.next().unwrap_or("");

    if parts.next().is_some() || middle.is_empty() {
        return Ok(false);
    }

    if pid.is_empty() || timestamp.is_empty() {
        return Ok(false);
    }

    if pid.len() > MAX_IDENTITY_KEY_TMP_NUMERIC_PART_BYTES
        || timestamp.len() > MAX_IDENTITY_KEY_TMP_NUMERIC_PART_BYTES
    {
        return Ok(false);
    }

    if !pid.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(false);
    }

    if !timestamp.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(false);
    }

    Ok(true)
}

async fn remove_file_if_exists(path: &Path) -> AgentResult<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_error) => Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::FileDeleteFailed,
        )),
    }
}

async fn create_secret_file_new(path: &Path, contents: &[u8]) -> AgentResult<()> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    set_secret_creation_mode(&mut options);
    let mut file = options
        .open(path)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    file.write_all(contents)
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))?;
    file.sync_data()
        .await
        .map_err(|_error| HephaestusAgentError::new(HephaestusAgentErrorReason::FileWriteFailed))
}

#[cfg(unix)]
fn set_secret_creation_mode(options: &mut tokio::fs::OpenOptions) {
    options.mode(0o600);
}

#[cfg(not(unix))]
fn set_secret_creation_mode(_options: &mut tokio::fs::OpenOptions) {}

fn validate_absolute_path(path: &Path) -> AgentResult<()> {
    if !path.is_absolute() {
        return Err(HephaestusAgentError::new(
            HephaestusAgentErrorReason::InvalidPath,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
