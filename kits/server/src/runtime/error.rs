// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::SocketAddr;

use reallyme_app_kit::{AppConfigDocumentErrorReason, AppKitErrorReason, AppKitField};
use thiserror::Error;

use crate::observability::ObservabilityError;
use crate::shutdown::ShutdownError;
use crate::startup::StartupError;
use crate::startup::TaskName;
use crate::task::TaskExecutionErrorKind;

/// Transport listener or serving surface managed by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerRuntimeTransport {
    /// HTTP/JSON transport.
    Http,
    /// gRPC transport.
    Grpc,
}

/// Low-cardinality server runtime failure kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerRuntimeErrorKind {
    /// Observability setup failed.
    Observability,
    /// Startup validation failed.
    Startup,
    /// A listener could not be bound.
    ListenerBind,
    /// A app-provided startup check failed.
    StartupCheck,
    /// Runtime app composition was invalid.
    AppComposition,
    /// Runtime listener composition was invalid.
    ListenerComposition,
    /// Runtime app cleanup failed.
    AppCleanup,
    /// Background task registration failed.
    TaskRegistration,
    /// Shutdown coordination failed.
    Shutdown,
    /// Required runtime input was not provided.
    MissingRequiredField,
}

/// Operational source classification for runtime failures.
///
/// This captures the useful part of Pingora's error taxonomy while preserving
/// ReallyMe's stricter typed-error policy: sources are low-cardinality enums,
/// not erased boxed errors or string context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationalFailureSource {
    /// Runtime framework or process orchestration failed.
    Runtime,
    /// App-provided behavior failed at a runtime boundary.
    App,
    /// Transport listener or serving infrastructure failed.
    Transport,
    /// Startup/configuration input failed validation.
    Configuration,
}

/// Retry guidance for operational runtime failures.
///
/// This is deliberately coarse and safe for logs/metrics. It must not encode
/// product-specific retry policy or leak internal details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDisposition {
    /// Retrying the same startup/shutdown action without changing inputs is not expected to help.
    NotRetryable,
    /// Retrying may succeed if the process/runtime condition changes.
    Retryable,
    /// Retrying may succeed only after validated configuration or composition changes.
    RetryAfterConfigurationChange,
}

/// Typed server runtime orchestration failures.
#[derive(Debug, Error)]
pub enum ServerRuntimeError {
    /// Observability initialization failed.
    #[error("server runtime observability initialization failed")]
    Observability {
        /// Typed observability source error.
        source: ObservabilityError,
    },
    /// Startup validation failed.
    #[error("server runtime startup validation failed")]
    Startup {
        /// Typed startup source error.
        source: StartupError,
    },
    /// Startup banner output failed.
    #[error("server runtime startup banner write failed")]
    StartupBannerWriteFailed,
    /// A transport listener could not be bound.
    #[error("server runtime listener bind failed")]
    ListenerBind {
        /// Transport that failed to bind.
        transport: ServerRuntimeTransport,
        /// Validated socket address the runtime attempted to bind.
        bind_address: SocketAddr,
        /// Low-cardinality operating-system bind failure reason.
        reason: RuntimeListenerBindErrorReason,
    },
    /// A app-provided startup check failed before readiness.
    #[error("runtime app startup check failed")]
    StartupCheck {
        /// Startup check name.
        check_name: TaskName,
        /// Low-cardinality failure kind.
        kind: TaskExecutionErrorKind,
    },
    /// Runtime app composition failed validation.
    #[error("server runtime app composition failed validation")]
    AppComposition {
        /// Typed app-composition failure reason.
        reason: RuntimeAppCompositionErrorReason,
    },
    /// Runtime listener composition failed validation.
    #[error("server runtime listener composition failed validation")]
    ListenerComposition {
        /// Typed listener-composition failure reason.
        reason: RuntimeListenerCompositionErrorReason,
    },
    /// A app-provided cleanup hook failed or exceeded its deadline.
    #[error("runtime app cleanup failed")]
    AppCleanup {
        /// Cleanup hook name.
        hook_name: TaskName,
        /// Typed cleanup failure reason.
        reason: RuntimeAppCleanupErrorReason,
    },
    /// A managed background task could not be registered.
    #[error("server runtime background task registration failed")]
    TaskRegistration {
        /// Typed shutdown/task-registration error.
        source: ShutdownError,
    },
    /// Coordinated shutdown failed.
    #[error("server runtime shutdown failed")]
    Shutdown {
        /// Typed shutdown source error.
        source: ShutdownError,
    },
    /// A required builder field was omitted.
    #[error("server runtime builder is missing a required field")]
    MissingRequiredField {
        /// Missing field.
        field: ServerRuntimeRequiredField,
    },
}

impl ServerRuntimeError {
    /// Returns the low-cardinality runtime failure kind.
    pub const fn kind(&self) -> ServerRuntimeErrorKind {
        match self {
            Self::Observability { .. } => ServerRuntimeErrorKind::Observability,
            Self::Startup { .. } | Self::StartupBannerWriteFailed => {
                ServerRuntimeErrorKind::Startup
            }
            Self::ListenerBind { .. } => ServerRuntimeErrorKind::ListenerBind,
            Self::StartupCheck { .. } => ServerRuntimeErrorKind::StartupCheck,
            Self::AppComposition { .. } => ServerRuntimeErrorKind::AppComposition,
            Self::ListenerComposition { .. } => ServerRuntimeErrorKind::ListenerComposition,
            Self::AppCleanup { .. } => ServerRuntimeErrorKind::AppCleanup,
            Self::TaskRegistration { .. } => ServerRuntimeErrorKind::TaskRegistration,
            Self::Shutdown { .. } => ServerRuntimeErrorKind::Shutdown,
            Self::MissingRequiredField { .. } => ServerRuntimeErrorKind::MissingRequiredField,
        }
    }

    /// Returns the low-cardinality operational failure source.
    pub const fn operational_source(&self) -> OperationalFailureSource {
        match self {
            Self::Observability { .. }
            | Self::StartupBannerWriteFailed
            | Self::TaskRegistration { .. }
            | Self::Shutdown { .. } => OperationalFailureSource::Runtime,
            Self::Startup { .. }
            | Self::AppComposition { .. }
            | Self::ListenerComposition { .. }
            | Self::MissingRequiredField { .. } => OperationalFailureSource::Configuration,
            Self::ListenerBind { .. } => OperationalFailureSource::Transport,
            Self::StartupCheck { .. } | Self::AppCleanup { .. } => OperationalFailureSource::App,
        }
    }

    /// Returns coarse retry guidance for this runtime failure.
    pub const fn retry_disposition(&self) -> RetryDisposition {
        match self {
            Self::ListenerBind { .. } | Self::Shutdown { .. } => RetryDisposition::Retryable,
            Self::Startup { .. }
            | Self::AppComposition { .. }
            | Self::ListenerComposition { .. }
            | Self::MissingRequiredField { .. } => RetryDisposition::RetryAfterConfigurationChange,
            Self::Observability { .. }
            | Self::StartupBannerWriteFailed
            | Self::StartupCheck { .. }
            | Self::AppCleanup { .. }
            | Self::TaskRegistration { .. } => RetryDisposition::NotRetryable,
        }
    }
}

/// Low-cardinality listener bind failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeListenerBindErrorReason {
    /// The address is already in use by another process or socket state.
    AddressInUse,
    /// The process does not have permission to bind the requested address.
    PermissionDenied,
    /// The requested local address is not available on this host.
    AddressNotAvailable,
    /// The operating system returned a different bind failure.
    Other,
}

impl RuntimeListenerBindErrorReason {
    /// Converts a standard I/O error kind into a stable runtime reason.
    pub const fn from_io_error_kind(kind: std::io::ErrorKind) -> Self {
        match kind {
            std::io::ErrorKind::AddrInUse => Self::AddressInUse,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::AddrNotAvailable => Self::AddressNotAvailable,
            _ => Self::Other,
        }
    }
}

/// Typed runtime app-composition validation reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAppCompositionErrorReason {
    /// The runtime was given the same app name more than once.
    DuplicateAppName,
    /// The runtime was given two apps mounted at the same HTTP path.
    DuplicateHttpMount,
    /// A runtime app depends on an app that is not registered in the runtime.
    UnknownDependency,
    /// A runtime app lists the same dependency more than once.
    DuplicateDependency,
    /// Runtime app dependencies contain a cycle.
    DependencyCycle,
}

/// Typed runtime listener-composition validation reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeListenerCompositionErrorReason {
    /// Two runtime listeners were configured with the same network port.
    DuplicateListenerPort,
    /// Two HTTP listeners were configured with the same listener name.
    DuplicateHttpListenerName,
}

/// Typed runtime app cleanup failure reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAppCleanupErrorReason {
    /// Cleanup exceeded the bounded runtime cleanup timeout.
    TimedOut,
    /// Cleanup returned a typed operational failure.
    Failed {
        /// Low-cardinality failure kind.
        kind: TaskExecutionErrorKind,
    },
}

/// Generic error used by app-owned native-server adapters.
///
/// App crates should use this instead of defining one adapter error enum per
/// app unless they have genuinely app-specific server-adapter failure modes.
/// The type deliberately stores only low-cardinality typed reasons from
/// app-kit/server-kit and never raw config values or framework debug output.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAppAdapterError {
    /// Host-neutral app metadata failed validation.
    #[error("runtime app metadata failed validation")]
    AppMetadata {
        /// App-kit field that failed validation.
        field: AppKitField,
        /// App-kit validation reason.
        reason: AppKitErrorReason,
    },
    /// Server-runtime app registration failed validation.
    #[error("runtime app server registration failed validation")]
    RuntimeRegistration {
        /// Server-kit startup validation failure.
        source: StartupError,
    },
    /// App JSONC config document failed validation.
    #[error("runtime app config document failed validation")]
    AppConfigDocument {
        /// App-kit config validation reason.
        reason: AppConfigDocumentErrorReason,
    },
}

impl From<reallyme_app_kit::AppKitError> for RuntimeAppAdapterError {
    fn from(error: reallyme_app_kit::AppKitError) -> Self {
        Self::AppMetadata {
            field: error.field(),
            reason: error.reason(),
        }
    }
}

impl From<StartupError> for RuntimeAppAdapterError {
    fn from(source: StartupError) -> Self {
        Self::RuntimeRegistration { source }
    }
}

impl From<reallyme_app_kit::AppConfigDocumentError> for RuntimeAppAdapterError {
    fn from(error: reallyme_app_kit::AppConfigDocumentError) -> Self {
        Self::AppConfigDocument {
            reason: error.reason(),
        }
    }
}

impl From<ObservabilityError> for ServerRuntimeError {
    fn from(source: ObservabilityError) -> Self {
        Self::Observability { source }
    }
}

impl From<StartupError> for ServerRuntimeError {
    fn from(source: StartupError) -> Self {
        Self::Startup { source }
    }
}

/// Required server runtime builder fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerRuntimeRequiredField {
    /// Server process name.
    ServerName,
    /// Observability configuration.
    ObservabilityConfig,
    /// Build information.
    BuildInfo,
    /// Readiness state.
    Readiness,
    /// Shutdown timeout.
    ShutdownTimeout,
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
