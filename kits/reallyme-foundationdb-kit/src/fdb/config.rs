// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Validated FoundationDB connection configuration.

use std::env::{self, VarError};
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::fdb::error::{ConfigErrorReason, ConfigField, FdbError, FdbResult};

const DEFAULT_API_VERSION: i32 = 730;
// The Rust binding generates version-gated tenant-management behavior at
// compile time. Allowing a runtime API that differs from the generated binding
// can select incompatible system-key layouts, so production configuration
// must match the compiled API exactly.
const MIN_API_VERSION: i32 = 730;
const MAX_API_VERSION: i32 = 730;
const DEFAULT_HEALTH_CHECK_TIMEOUT_MILLIS: u64 = 5_000;
const MAX_HEALTH_CHECK_TIMEOUT_MILLIS: u64 = 60_000;
const MAX_CLUSTER_FILE_PATH_BYTES: usize = 4_096;
const MAX_ENVIRONMENT_PREFIX_BYTES: usize = 128;

/// Raw FoundationDB connector configuration input.
#[derive(Clone, PartialEq, Eq)]
pub struct FdbConfigInput {
    /// Optional FoundationDB cluster-file path.
    pub fdb_cluster_file: Option<String>,
    /// FoundationDB C API compatibility version selected for the process.
    pub fdb_api_version: i32,
    /// Deadline for connector health and startup connectivity checks.
    pub health_check_timeout_millis: u64,
}

impl Default for FdbConfigInput {
    fn default() -> Self {
        Self {
            fdb_cluster_file: None,
            fdb_api_version: DEFAULT_API_VERSION,
            health_check_timeout_millis: DEFAULT_HEALTH_CHECK_TIMEOUT_MILLIS,
        }
    }
}

impl std::fmt::Debug for FdbConfigInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FdbConfigInput")
            .field(
                "fdb_cluster_file",
                &self.fdb_cluster_file.as_ref().map(|_| "<configured>"),
            )
            .field("fdb_api_version", &self.fdb_api_version)
            .field(
                "health_check_timeout_millis",
                &self.health_check_timeout_millis,
            )
            .finish()
    }
}

/// Validated FoundationDB connector configuration.
///
/// The cluster-file path is deliberately redacted from `Debug` output. Cluster
/// files are not credentials, but their paths commonly disclose deployment
/// topology and should not become routine log metadata.
#[derive(Clone)]
pub struct FdbConfig {
    fdb_cluster_file: Option<String>,
    fdb_api_version: i32,
    health_check_timeout: Duration,
}

impl FdbConfig {
    /// Constructs validated connector configuration.
    pub fn new(input: FdbConfigInput) -> FdbResult<Self> {
        validate_api_version(input.fdb_api_version)?;
        validate_positive_bounded_u64(
            input.health_check_timeout_millis,
            MAX_HEALTH_CHECK_TIMEOUT_MILLIS,
            ConfigField::HealthCheckTimeoutMillis,
        )?;
        let fdb_cluster_file = input
            .fdb_cluster_file
            .map(validate_cluster_file_path)
            .transpose()?;

        Ok(Self {
            fdb_cluster_file,
            fdb_api_version: input.fdb_api_version,
            health_check_timeout: Duration::from_millis(input.health_check_timeout_millis),
        })
    }

    /// Builds configuration from the conventional process environment.
    ///
    /// Reads `FDB_CLUSTER_FILE`, `FDB_API_VERSION`, and
    /// `FDB_HEALTH_CHECK_TIMEOUT_MILLIS`.
    pub fn from_env() -> FdbResult<Self> {
        Self::from_env_names(
            "FDB_CLUSTER_FILE",
            "FDB_API_VERSION",
            "FDB_HEALTH_CHECK_TIMEOUT_MILLIS",
        )
    }

    /// Builds configuration from service-prefixed environment variables.
    ///
    /// For `prefix = "HANDLE"`, this reads `HANDLE_FDB_CLUSTER_FILE`,
    /// `HANDLE_FDB_API_VERSION`, and
    /// `HANDLE_FDB_HEALTH_CHECK_TIMEOUT_MILLIS`. FoundationDB supports one
    /// client network per process, so one prefix must be selected per process.
    pub fn from_env_prefix(prefix: &str) -> FdbResult<Self> {
        let cluster_file = env_name(prefix, "FDB_CLUSTER_FILE")?;
        let api_version = env_name(prefix, "FDB_API_VERSION")?;
        let health_timeout = env_name(prefix, "FDB_HEALTH_CHECK_TIMEOUT_MILLIS")?;
        Self::from_env_names(&cluster_file, &api_version, &health_timeout)
    }

    fn from_env_names(
        cluster_file_name: &str,
        api_version_name: &str,
        health_timeout_name: &str,
    ) -> FdbResult<Self> {
        Self::new(FdbConfigInput {
            fdb_cluster_file: optional_env(cluster_file_name, ConfigField::ClusterFile)?,
            fdb_api_version: parse_env_i32(
                api_version_name,
                DEFAULT_API_VERSION,
                ConfigField::ApiVersion,
            )?,
            health_check_timeout_millis: parse_env_u64(
                health_timeout_name,
                DEFAULT_HEALTH_CHECK_TIMEOUT_MILLIS,
                ConfigField::HealthCheckTimeoutMillis,
            )?,
        })
    }

    /// Returns the optional validated cluster-file path.
    pub fn fdb_cluster_file(&self) -> Option<&str> {
        self.fdb_cluster_file.as_deref()
    }

    /// Returns the configured FoundationDB runtime API version.
    pub const fn fdb_api_version(&self) -> i32 {
        self.fdb_api_version
    }

    /// Returns the deadline applied to a complete connector health probe.
    pub const fn health_check_timeout(&self) -> Duration {
        self.health_check_timeout
    }
}

impl std::fmt::Debug for FdbConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FdbConfig")
            .field(
                "fdb_cluster_file",
                &self.fdb_cluster_file.as_ref().map(|_| "<configured>"),
            )
            .field("fdb_api_version", &self.fdb_api_version)
            .field("health_check_timeout", &self.health_check_timeout)
            .finish()
    }
}

fn validate_api_version(value: i32) -> FdbResult<()> {
    if !(MIN_API_VERSION..=MAX_API_VERSION).contains(&value) {
        return Err(config_error(ConfigErrorReason::ValueOutOfBounds {
            field: ConfigField::ApiVersion,
        }));
    }
    Ok(())
}

fn validate_cluster_file_path(path: String) -> FdbResult<String> {
    if path.is_empty() {
        return Err(config_error(ConfigErrorReason::Empty));
    }
    if path.len() > MAX_CLUSTER_FILE_PATH_BYTES || path.contains('\0') {
        return Err(config_error(ConfigErrorReason::InvalidClusterFilePath));
    }

    let file = File::open(Path::new(&path))
        .map_err(|_| config_error(ConfigErrorReason::MissingClusterFile))?;
    let metadata = file
        .metadata()
        .map_err(|_| config_error(ConfigErrorReason::MissingClusterFile))?;
    if !metadata.is_file() {
        return Err(config_error(ConfigErrorReason::InvalidClusterFilePath));
    }

    Ok(path)
}

fn validate_positive_bounded_u64(value: u64, maximum: u64, field: ConfigField) -> FdbResult<()> {
    if value == 0 || value > maximum {
        return Err(config_error(ConfigErrorReason::ValueOutOfBounds { field }));
    }
    Ok(())
}

fn env_name(prefix: &str, suffix: &str) -> FdbResult<String> {
    if prefix.is_empty()
        || prefix.len() > MAX_ENVIRONMENT_PREFIX_BYTES
        || !prefix
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(config_error(ConfigErrorReason::InvalidEnvironmentPrefix));
    }
    Ok(format!("{prefix}_{suffix}"))
}

fn optional_env(name: &str, field: ConfigField) -> FdbResult<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(_)) => {
            Err(config_error(ConfigErrorReason::InvalidEncoding { field }))
        }
    }
}

fn parse_env_i32(name: &str, default: i32, field: ConfigField) -> FdbResult<i32> {
    optional_env(name, field)?
        .map(|value| {
            value
                .parse::<i32>()
                .map_err(|_| config_error(ConfigErrorReason::InvalidInteger { field }))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_env_u64(name: &str, default: u64, field: ConfigField) -> FdbResult<u64> {
    optional_env(name, field)?
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| config_error(ConfigErrorReason::InvalidInteger { field }))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

const fn config_error(reason: ConfigErrorReason) -> FdbError {
    FdbError::Config { reason }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};

    use temp_env::with_vars;

    use super::{FdbConfig, FdbConfigInput, env_name};
    use crate::fdb::error::{ConfigErrorReason, ConfigField, FdbError};

    #[test]
    fn config_defaults_apply_when_env_is_unset() {
        with_vars(
            [
                ("FDB_API_VERSION", None::<&str>),
                ("FDB_CLUSTER_FILE", None::<&str>),
                ("FDB_HEALTH_CHECK_TIMEOUT_MILLIS", None::<&str>),
            ],
            || {
                let result = FdbConfig::from_env();
                assert!(result.is_ok());
                let config = result.unwrap_or_else(|_| unreachable!());
                assert_eq!(config.fdb_api_version(), 730);
                assert_eq!(config.health_check_timeout().as_millis(), 5_000);
                assert!(config.fdb_cluster_file().is_none());
            },
        );
    }

    #[test]
    fn config_rejects_empty_cluster_file_path() {
        let result = FdbConfig::new(FdbConfigInput {
            fdb_cluster_file: Some(String::new()),
            ..FdbConfigInput::default()
        });
        assert!(matches!(
            result,
            Err(FdbError::Config {
                reason: ConfigErrorReason::Empty,
            })
        ));
    }

    #[test]
    fn config_rejects_missing_cluster_file_path() {
        let result = FdbConfig::new(FdbConfigInput {
            fdb_cluster_file: Some("/tmp/__reallyme_does_not_exist".to_owned()),
            ..FdbConfigInput::default()
        });
        assert!(matches!(
            result,
            Err(FdbError::Config {
                reason: ConfigErrorReason::MissingClusterFile,
            })
        ));
    }

    #[test]
    fn config_validates_existing_cluster_file_path() {
        let mut path = std::env::temp_dir();
        path.push(format!("reallyme-fdb-cluster-{}.conf", std::process::id()));
        let created = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map(|_| ());
        assert!(created.is_ok());

        let result = path.to_str().map(|value| {
            FdbConfig::new(FdbConfigInput {
                fdb_cluster_file: Some(value.to_owned()),
                ..FdbConfigInput::default()
            })
        });
        assert!(result.is_some_and(|value| value.is_ok()));
        assert!(fs::remove_file(path).is_ok());
    }

    #[test]
    fn health_check_timeout_is_bounded() {
        for timeout in [0, 60_001] {
            let result = FdbConfig::new(FdbConfigInput {
                health_check_timeout_millis: timeout,
                ..FdbConfigInput::default()
            });
            assert!(matches!(
                result,
                Err(FdbError::Config {
                    reason: ConfigErrorReason::ValueOutOfBounds {
                        field: ConfigField::HealthCheckTimeoutMillis,
                    },
                })
            ));
        }
    }

    #[test]
    fn runtime_api_must_match_compiled_foundationdb_api() {
        let result = FdbConfig::new(FdbConfigInput {
            fdb_api_version: 740,
            ..FdbConfigInput::default()
        });
        assert!(matches!(
            result,
            Err(FdbError::Config {
                reason: ConfigErrorReason::ValueOutOfBounds {
                    field: ConfigField::ApiVersion,
                },
            })
        ));
    }

    #[test]
    fn cluster_file_path_length_is_bounded_before_file_access() {
        let result = FdbConfig::new(FdbConfigInput {
            fdb_cluster_file: Some("x".repeat(4_097)),
            ..FdbConfigInput::default()
        });
        assert!(matches!(
            result,
            Err(FdbError::Config {
                reason: ConfigErrorReason::InvalidClusterFilePath,
            })
        ));
    }

    #[test]
    fn environment_prefix_is_strictly_validated() {
        assert_eq!(
            env_name("HANDLE", "FDB_API_VERSION").as_deref(),
            Ok("HANDLE_FDB_API_VERSION")
        );
        for invalid_prefix in ["handle", "", "HANDLE-DATA"] {
            assert!(matches!(
                env_name(invalid_prefix, "FDB_API_VERSION"),
                Err(FdbError::Config {
                    reason: ConfigErrorReason::InvalidEnvironmentPrefix,
                })
            ));
        }
    }

    #[test]
    fn api_version_must_parse_and_stay_in_supported_range() {
        with_vars([("FDB_API_VERSION", Some("not-a-number"))], || {
            assert!(matches!(
                FdbConfig::from_env(),
                Err(FdbError::Config {
                    reason: ConfigErrorReason::InvalidInteger {
                        field: ConfigField::ApiVersion,
                    },
                })
            ));
        });
        with_vars([("FDB_API_VERSION", Some("100"))], || {
            assert!(matches!(
                FdbConfig::from_env(),
                Err(FdbError::Config {
                    reason: ConfigErrorReason::ValueOutOfBounds {
                        field: ConfigField::ApiVersion,
                    },
                })
            ));
        });
    }

    #[test]
    fn debug_output_redacts_cluster_file_path() {
        let input = FdbConfigInput {
            fdb_cluster_file: Some("/sensitive/deployment/topology/fdb.cluster".to_owned()),
            ..FdbConfigInput::default()
        };
        let rendered = format!("{input:?}");
        assert!(!rendered.contains("sensitive"));
        assert!(rendered.contains("<configured>"));
    }
}
