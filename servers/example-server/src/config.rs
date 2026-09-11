// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::path::Path;
use std::time::Duration;

use reallyme_app_kit::{AppConfigProfile, parse_jsonc_config};
use reallyme_server_kit::config::{
    BindAddress, BodyLimitConfig, CorsConfig, HttpServerConfig, LogFormat, MetricsIdleTimeout,
    ObservabilityConfig, RequestBodyLimitBytes, RequestTimeout, ServiceEnvironment, TimeoutConfig,
};
use reallyme_server_kit::task::ShutdownTimeout;
use serde::Deserialize;

use crate::error::{ExampleServerError, ExampleServerErrorReason};

const MAX_SERVER_CONFIG_BYTES: u64 = 1_048_576;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExampleServerConfig {
    application_profile: AppConfigProfile,
    bind_address: SocketAddr,
    request_timeout_seconds: u64,
    request_body_limit_bytes: usize,
    metrics_idle_timeout_seconds: u64,
    shutdown_timeout_seconds: u64,
}

pub(crate) struct ExampleServerConfig {
    application_profile: AppConfigProfile,
    http: HttpServerConfig,
    observability: ObservabilityConfig,
    shutdown_timeout: ShutdownTimeout,
}

impl ExampleServerConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, ExampleServerError> {
        let file = File::open(path).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileUnreadable)
        })?;
        let read_limit = MAX_SERVER_CONFIG_BYTES.checked_add(1).ok_or_else(|| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileTooLarge)
        })?;
        let initial_capacity = usize::try_from(read_limit).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileTooLarge)
        })?;
        let mut bytes = Vec::with_capacity(initial_capacity);
        file.take(read_limit).read_to_end(&mut bytes).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileUnreadable)
        })?;

        if u64::try_from(bytes.len()).map_or(true, |length| length > MAX_SERVER_CONFIG_BYTES) {
            return Err(ExampleServerError::new(
                ExampleServerErrorReason::ServerConfigFileTooLarge,
            ));
        }

        let document = String::from_utf8(bytes).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigFileNotUtf8)
        })?;
        Self::from_jsonc_str(document.as_str())
    }

    fn from_jsonc_str(value: &str) -> Result<Self, ExampleServerError> {
        let raw = parse_jsonc_config::<RawExampleServerConfig>(value).map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ServerConfigDocumentInvalid)
        })?;
        let bind_address = BindAddress::new(raw.bind_address)
            .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::BindAddressInvalid))?;
        let request_timeout = RequestTimeout::new(Duration::from_secs(raw.request_timeout_seconds))
            .map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::RequestTimeoutInvalid)
            })?;
        let request_body_limit =
            RequestBodyLimitBytes::new(raw.request_body_limit_bytes).map_err(|_| {
                ExampleServerError::new(ExampleServerErrorReason::RequestBodyLimitInvalid)
            })?;
        let metrics_idle_timeout =
            MetricsIdleTimeout::new(Duration::from_secs(raw.metrics_idle_timeout_seconds))
                .map_err(|_| {
                    ExampleServerError::new(ExampleServerErrorReason::MetricsIdleTimeoutInvalid)
                })?;
        let environment = environment_for_profile(raw.application_profile);
        let log_format = match environment {
            ServiceEnvironment::Local | ServiceEnvironment::Dev => LogFormat::PlainText,
            ServiceEnvironment::Staging | ServiceEnvironment::Prod => LogFormat::Json,
        };
        let observability = ObservabilityConfig::new(
            environment,
            log_format,
            false,
            "info".to_owned(),
            metrics_idle_timeout,
        )
        .map_err(|_| {
            ExampleServerError::new(ExampleServerErrorReason::ObservabilityConfigInvalid)
        })?;
        let shutdown_timeout = ShutdownTimeout::new(Duration::from_secs(
            raw.shutdown_timeout_seconds,
        ))
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::ShutdownTimeoutInvalid))?;

        Ok(Self {
            application_profile: raw.application_profile,
            http: HttpServerConfig::new(
                bind_address,
                CorsConfig::no_cors(),
                TimeoutConfig::new(request_timeout),
                BodyLimitConfig::new(request_body_limit),
            ),
            observability,
            shutdown_timeout,
        })
    }

    pub(crate) const fn application_profile(&self) -> AppConfigProfile {
        self.application_profile
    }

    pub(crate) fn http(&self) -> &HttpServerConfig {
        &self.http
    }

    pub(crate) fn observability(&self) -> &ObservabilityConfig {
        &self.observability
    }

    pub(crate) const fn shutdown_timeout(&self) -> ShutdownTimeout {
        self.shutdown_timeout
    }
}

const fn environment_for_profile(profile: AppConfigProfile) -> ServiceEnvironment {
    match profile {
        AppConfigProfile::Local => ServiceEnvironment::Local,
        AppConfigProfile::Staging => ServiceEnvironment::Staging,
        AppConfigProfile::Prod => ServiceEnvironment::Prod,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{IpAddr, Ipv4Addr};

    use reallyme_app_kit::AppConfigProfile;
    use thiserror::Error;

    use super::{ExampleServerConfig, MAX_SERVER_CONFIG_BYTES};
    use crate::error::ExampleServerErrorReason;

    #[derive(Debug, Error)]
    enum ConfigTestError {
        #[error("temporary config file operation failed")]
        File(#[from] std::io::Error),
        #[error("test fixture length is not representable")]
        FixtureLength,
    }

    #[test]
    fn checked_in_local_config_is_valid_and_loopback_only()
    -> Result<(), crate::error::ExampleServerError> {
        let config = ExampleServerConfig::from_jsonc_str(include_str!(
            "../../configs/example-server.jsonc"
        ))?;

        assert_eq!(config.application_profile(), AppConfigProfile::Local);
        assert_eq!(
            config.http().bind_address().ip_addr(),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
        assert_eq!(config.http().bind_address().port().as_u16(), 8080);
        Ok(())
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let result = ExampleServerConfig::from_jsonc_str(
            r#"{
                "application_profile": "local",
                "bind_address": "127.0.0.1:8080",
                "request_timeout_seconds": 30,
                "request_body_limit_bytes": 1048576,
                "metrics_idle_timeout_seconds": 30,
                "shutdown_timeout_seconds": 10,
                "unexpected": true
            }"#,
        );

        assert!(matches!(
            result,
            Err(error)
                if error.reason() == ExampleServerErrorReason::ServerConfigDocumentInvalid
        ));
    }

    #[test]
    fn zero_request_timeout_is_rejected() {
        let result = ExampleServerConfig::from_jsonc_str(
            r#"{
                "application_profile": "local",
                "bind_address": "127.0.0.1:8080",
                "request_timeout_seconds": 0,
                "request_body_limit_bytes": 1048576,
                "metrics_idle_timeout_seconds": 30,
                "shutdown_timeout_seconds": 10
            }"#,
        );

        assert!(matches!(
            result,
            Err(error) if error.reason() == ExampleServerErrorReason::RequestTimeoutInvalid
        ));
    }

    #[test]
    fn non_utf8_config_file_is_rejected() -> Result<(), ConfigTestError> {
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(&[0xff])?;

        let result = ExampleServerConfig::load(file.path());

        assert!(matches!(
            result,
            Err(error) if error.reason() == ExampleServerErrorReason::ServerConfigFileNotUtf8
        ));
        Ok(())
    }

    #[test]
    fn oversized_config_file_is_rejected_before_parsing() -> Result<(), ConfigTestError> {
        let oversized_length = MAX_SERVER_CONFIG_BYTES
            .checked_add(1)
            .and_then(|length| usize::try_from(length).ok())
            .ok_or(ConfigTestError::FixtureLength)?;
        let mut file = tempfile::NamedTempFile::new()?;
        file.write_all(vec![b' '; oversized_length].as_slice())?;

        let result = ExampleServerConfig::load(file.path());

        assert!(matches!(
            result,
            Err(error) if error.reason() == ExampleServerErrorReason::ServerConfigFileTooLarge
        ));
        Ok(())
    }
}
