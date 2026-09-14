// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::str::FromStr;
use std::time::Duration;

use crate::config::{
    BodyLimitConfig, ConfigCriticality, ConfigError, ConfigValueErrorReason, CorsConfig,
    EnvVarName, FromEnvironment, GrpcServerConfig, HttpServerConfig, MetricsIdleTimeout,
    ObservabilityConfig, ProcessEnvironment, RequestBodyLimitBytes, RequestTimeout, TimeoutConfig,
    ValidateConfig, load_var_with_criticality, optional_var, required_var,
};
use crate::config::{Secret, SecretString};

fn parse_required_for_test<T, E>(
    environment: &E,
    name: EnvVarName,
    reason: ConfigValueErrorReason,
) -> Result<T, ConfigError>
where
    T: FromStr,
    E: crate::config::EnvironmentProvider,
{
    let value = required_var(environment, name)?;
    value
        .parse::<T>()
        .map_err(|_| ConfigError::InvalidValue { name, reason })
}

fn parse_optional_for_test<T, E>(
    environment: &E,
    name: EnvVarName,
    reason: ConfigValueErrorReason,
) -> Result<Option<T>, ConfigError>
where
    T: FromStr,
    E: crate::config::EnvironmentProvider,
{
    match optional_var(environment, name)? {
        Some(value) => value
            .parse::<T>()
            .map(Some)
            .map_err(|_| ConfigError::InvalidValue { name, reason }),
        None => Ok(None),
    }
}

pub(crate) fn test_env_var_name(value: &'static str) -> EnvVarName {
    EnvVarName::new(value).expect("valid test environment variable name")
}

#[derive(Debug)]
pub(crate) struct TestServiceConfig {
    pub(crate) http: HttpServerConfig,
    pub(crate) grpc: GrpcServerConfig,
    pub(crate) observability: ObservabilityConfig,
    pub(crate) api_token: SecretString,
}

impl FromEnvironment for TestServiceConfig {
    fn from_environment<E>(environment: &E) -> Result<Self, ConfigError>
    where
        E: crate::config::EnvironmentProvider,
    {
        let service_environment_name = test_env_var_name("TEST_SERVICE_ENVIRONMENT");
        let http_bind_name = test_env_var_name("TEST_HTTP_BIND_ADDR");
        let grpc_bind_name = test_env_var_name("TEST_GRPC_BIND_ADDR");
        let timeout_name = test_env_var_name("TEST_REQUEST_TIMEOUT_MILLIS");
        let body_limit_name = test_env_var_name("TEST_REQUEST_BODY_LIMIT_BYTES");
        let log_format_name = test_env_var_name("TEST_LOG_FORMAT");
        let tracing_filter_name = test_env_var_name("TEST_TRACING_FILTER");
        let emit_span_events_name = test_env_var_name("TEST_EMIT_SPAN_EVENTS");
        let metrics_idle_timeout_name = test_env_var_name("TEST_METRICS_IDLE_TIMEOUT_SECONDS");
        let api_token_name = test_env_var_name("TEST_API_TOKEN");

        let service_environment = parse_required_for_test(
            environment,
            service_environment_name,
            ConfigValueErrorReason::InvalidServiceEnvironment,
        )?;
        let http_bind_address = crate::config::BindAddress::new(parse_required_for_test(
            environment,
            http_bind_name,
            ConfigValueErrorReason::InvalidSocketAddress,
        )?)?;
        let grpc_bind_address = crate::config::BindAddress::new(parse_required_for_test(
            environment,
            grpc_bind_name,
            ConfigValueErrorReason::InvalidSocketAddress,
        )?)?;
        let request_timeout = RequestTimeout::new(Duration::from_millis(parse_required_for_test(
            environment,
            timeout_name,
            ConfigValueErrorReason::InvalidInteger,
        )?))?;
        let request_body_limit = RequestBodyLimitBytes::new(parse_required_for_test(
            environment,
            body_limit_name,
            ConfigValueErrorReason::InvalidInteger,
        )?)?;
        let log_format = parse_required_for_test(
            environment,
            log_format_name,
            ConfigValueErrorReason::InvalidLogFormat,
        )?;
        let emit_span_events = parse_optional_for_test(
            environment,
            emit_span_events_name,
            ConfigValueErrorReason::InvalidBoolean,
        )?
        .unwrap_or(crate::config::DEFAULT_EMIT_SPAN_EVENTS);
        let tracing_filter = required_var(environment, tracing_filter_name)?;
        let metrics_idle_timeout = match parse_optional_for_test::<u64, _>(
            environment,
            metrics_idle_timeout_name,
            ConfigValueErrorReason::InvalidInteger,
        )? {
            Some(seconds) => MetricsIdleTimeout::new(Duration::from_secs(seconds))?,
            None => MetricsIdleTimeout::new(crate::config::DEFAULT_METRICS_IDLE_TIMEOUT)
                .expect("server-kit default metrics idle timeout must remain valid"),
        };
        let api_token = load_var_with_criticality(
            environment,
            service_environment,
            api_token_name,
            ConfigCriticality::RequiredInProduction,
        )?;

        Ok(Self {
            http: HttpServerConfig::new(
                http_bind_address,
                CorsConfig::no_cors(),
                TimeoutConfig::new(request_timeout),
                BodyLimitConfig::new(request_body_limit),
            ),
            grpc: GrpcServerConfig::new(grpc_bind_address),
            observability: ObservabilityConfig::new(
                service_environment,
                log_format,
                emit_span_events,
                tracing_filter,
                metrics_idle_timeout,
            )?,
            api_token: Secret::new(api_token.unwrap_or_default()),
        })
    }
}

impl ValidateConfig for TestServiceConfig {
    fn validate(self) -> Result<Self, ConfigError> {
        Ok(self)
    }
}

pub(crate) fn load_process_test_config() -> Result<TestServiceConfig, ConfigError> {
    crate::config::load_env::<TestServiceConfig, _>(&ProcessEnvironment)
}
