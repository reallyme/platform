// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;

use super::{ConfigCriticality, ConfigError, EnvVarName, ServiceEnvironment};

/// Abstraction over a source of environment values.
///
/// Service-specific configuration types own their variable names and loading
/// policy. This trait allows them to remain testable without coupling unit
/// tests to process-global environment mutation.
///
/// This is a startup-time configuration boundary, not a runtime shared-state
/// service trait. It intentionally does not require `Send + Sync`; callers that
/// need to share an environment provider can wrap a concrete provider in their
/// own startup code without widening the server-kit API.
pub trait EnvironmentProvider {
    /// Returns the raw environment value if present.
    fn get(&self, name: EnvVarName) -> Result<Option<String>, ConfigError>;
}

/// Real process environment provider.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnvironment;

impl EnvironmentProvider for ProcessEnvironment {
    fn get(&self, name: EnvVarName) -> Result<Option<String>, ConfigError> {
        match env::var(name.as_str()) {
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(env::VarError::NotUnicode(_)) => Err(ConfigError::InvalidUnicode { name }),
        }
    }
}

/// Implemented by app-specific configuration types that load themselves
/// from an environment provider.
///
/// Implementations are expected to map parse and validation failures into
/// documented `ConfigError` variants without echoing raw environment values.
/// This keeps the public error surface stable and safe for startup logs while
/// still allowing services to classify failures with typed reasons.
pub trait FromEnvironment: Sized {
    /// Constructs the configuration object from the provided environment source.
    fn from_environment<E>(environment: &E) -> Result<Self, ConfigError>
    where
        E: EnvironmentProvider;
}

/// Implemented by configuration types that perform explicit post-load
/// validation or cross-field checks.
pub trait ValidateConfig: Sized {
    /// Validates the configuration and returns the validated value.
    fn validate(self) -> Result<Self, ConfigError>;
}

/// Loads and validates a strongly typed configuration value from an
/// environment source.
///
/// Source precedence is intentionally explicit and simple:
///
/// 1. service-defined constructor defaults in the loading type
/// 2. environment variables read through the provided `EnvironmentProvider`
/// 3. post-load validation through `ValidateConfig`
///
/// `reallyme-server-kit` does not define application-specific variable names
/// or prefixes. Services remain responsible for those choices.
///
/// # Examples
///
/// ```rust
/// use std::time::Duration;
///
/// use reallyme_server_kit::config::{
///     ConfigCriticality, ConfigError, ConfigValueErrorReason, EnvVarName, EnvironmentProvider,
///     FromEnvironment, RequestTimeout, SecretString, ServiceEnvironment, ValidateConfig,
///     load_from_environment, load_var_with_criticality, required_var,
/// };
///
/// #[derive(Debug)]
/// struct ExampleServiceConfig {
///     service_environment: ServiceEnvironment,
///     request_timeout: RequestTimeout,
///     api_token: Option<SecretString>,
/// }
///
/// impl FromEnvironment for ExampleServiceConfig {
///     fn from_environment<E>(environment: &E) -> Result<Self, ConfigError>
///     where
///         E: EnvironmentProvider,
///     {
///         let environment_name = EnvVarName::new("REALLYME_API_ENVIRONMENT")?;
///         let request_timeout_name = EnvVarName::new("REALLYME_API_REQUEST_TIMEOUT_MS")?;
///         let api_token_name = EnvVarName::new("REALLYME_API_TOKEN")?;
///
///         let service_environment = required_var(environment, environment_name)?
///             .parse()
///             .map_err(|_| ConfigError::InvalidValue {
///                 name: environment_name,
///                 reason: ConfigValueErrorReason::InvalidServiceEnvironment,
///             })?;
///
///         let request_timeout_ms = required_var(environment, request_timeout_name)?
///             .parse::<u64>()
///             .map_err(|_| ConfigError::InvalidValue {
///                 name: request_timeout_name,
///                 reason: ConfigValueErrorReason::InvalidInteger,
///             })?;
///
///         let api_token = load_var_with_criticality(
///             environment,
///             service_environment,
///             api_token_name,
///             ConfigCriticality::RequiredInProduction,
///         )?
///         .map(SecretString::new);
///
///         Ok(Self {
///             service_environment,
///             request_timeout: RequestTimeout::new(Duration::from_millis(request_timeout_ms))?,
///             api_token,
///         })
///     }
/// }
///
/// impl ValidateConfig for ExampleServiceConfig {
///     fn validate(self) -> Result<Self, ConfigError> {
///         Ok(self)
///     }
/// }
///
/// struct FakeEnvironment;
///
/// impl EnvironmentProvider for FakeEnvironment {
///     fn get(&self, name: EnvVarName) -> Result<Option<String>, ConfigError> {
///         let value = match name.as_str() {
///             "REALLYME_API_ENVIRONMENT" => Some("local"),
///             "REALLYME_API_REQUEST_TIMEOUT_MS" => Some("500"),
///             "REALLYME_API_TOKEN" => None,
///             _ => None,
///         };
///
///         Ok(value.map(str::to_owned))
///     }
/// }
///
/// let config = load_from_environment::<ExampleServiceConfig, _>(&FakeEnvironment)?;
///
/// assert_eq!(config.service_environment, ServiceEnvironment::Local);
/// assert_eq!(config.request_timeout.as_duration(), Duration::from_millis(500));
/// assert!(config.api_token.is_none());
/// #
/// # Ok::<(), ConfigError>(())
/// ```
pub fn load_from_environment<T, E>(environment: &E) -> Result<T, ConfigError>
where
    T: FromEnvironment + ValidateConfig,
    E: EnvironmentProvider,
{
    T::from_environment(environment)?.validate()
}

/// Shorthand for [`load_from_environment`].
///
/// The shorter name is retained for ergonomics, but the operation is scoped to
/// loading one typed configuration root from a provided environment source. It
/// does not snapshot or own the entire process environment.
pub fn load_env<T, E>(environment: &E) -> Result<T, ConfigError>
where
    T: FromEnvironment + ValidateConfig,
    E: EnvironmentProvider,
{
    load_from_environment(environment)
}

/// Loads a required environment variable as a `String`.
pub fn required_var<E>(environment: &E, name: EnvVarName) -> Result<String, ConfigError>
where
    E: EnvironmentProvider,
{
    match environment.get(name)? {
        Some(value) => Ok(value),
        None => Err(ConfigError::MissingRequiredVariable { name }),
    }
}

/// Loads an optional environment variable as a `String`.
pub fn optional_var<E>(environment: &E, name: EnvVarName) -> Result<Option<String>, ConfigError>
where
    E: EnvironmentProvider,
{
    environment.get(name)
}

/// Loads an environment variable according to an explicit criticality policy.
///
/// This allows service configuration loaders to fail closed for production
/// credentials or endpoints while still permitting local development defaults
/// for lower environments when appropriate.
pub fn load_var_with_criticality<E>(
    environment: &E,
    service_environment: ServiceEnvironment,
    name: EnvVarName,
    criticality: ConfigCriticality,
) -> Result<Option<String>, ConfigError>
where
    E: EnvironmentProvider,
{
    let value = optional_var(environment, name)?;

    match (criticality, value) {
        (ConfigCriticality::Optional, value) => Ok(value),
        (ConfigCriticality::Required, Some(value)) => Ok(Some(value)),
        (ConfigCriticality::Required, None) => Err(ConfigError::MissingRequiredVariable { name }),
        (ConfigCriticality::RequiredInProduction, Some(value)) => Ok(Some(value)),
        (ConfigCriticality::RequiredInProduction, None)
            if service_environment.is_production_like() =>
        {
            Err(ConfigError::MissingProductionCriticalVariable {
                name,
                service_environment,
            })
        }
        (ConfigCriticality::RequiredInProduction, None) => Ok(None),
    }
}
