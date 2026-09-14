// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

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
