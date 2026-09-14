// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Typed failure returned while configuring, composing, or running the example server.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("example server failed: {reason}")]
pub struct ExampleServerError {
    reason: ExampleServerErrorReason,
}

impl ExampleServerError {
    pub(crate) const fn new(reason: ExampleServerErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the stable, low-cardinality failure reason.
    pub const fn reason(self) -> ExampleServerErrorReason {
        self.reason
    }
}

/// Stable failure reasons for the example server host.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ExampleServerErrorReason {
    /// The required `--config <path>` command-line argument is absent.
    #[error("usage: example-server --config <path>")]
    ConfigArgumentMissing,
    /// The command line contains an unsupported flag or additional argument.
    #[error("usage: example-server --config <path>")]
    CommandLineInvalid,
    /// The selected server configuration file could not be opened or read.
    #[error("the server configuration file could not be read")]
    ServerConfigFileUnreadable,
    /// The selected server configuration file exceeds its bounded size limit.
    #[error("the server configuration file is too large")]
    ServerConfigFileTooLarge,
    /// The selected server configuration file is not valid UTF-8.
    #[error("the server configuration file is not UTF-8")]
    ServerConfigFileNotUtf8,
    /// The selected server JSONC document is malformed or has unknown fields.
    #[error("the server configuration document is invalid")]
    ServerConfigDocumentInvalid,
    /// The configured bind address violates server-kit network policy.
    #[error("the HTTP bind address is invalid")]
    BindAddressInvalid,
    /// The configured request timeout violates server-kit policy.
    #[error("the HTTP request timeout is invalid")]
    RequestTimeoutInvalid,
    /// The configured request body limit violates server-kit policy.
    #[error("the HTTP request body limit is invalid")]
    RequestBodyLimitInvalid,
    /// The configured metrics idle timeout violates server-kit policy.
    #[error("the metrics idle timeout is invalid")]
    MetricsIdleTimeoutInvalid,
    /// The host's observability configuration is invalid.
    #[error("the observability configuration is invalid")]
    ObservabilityConfigInvalid,
    /// The configured shutdown timeout violates server-kit policy.
    #[error("the shutdown timeout is invalid")]
    ShutdownTimeoutInvalid,
    /// A compile-time server or listener identity is invalid.
    #[error("a compiled server identity is invalid")]
    CompiledIdentityInvalid,
    /// The selected application's checked-in configuration is invalid.
    #[error("the example application configuration is invalid")]
    ApplicationConfigInvalid,
    /// The selected application could not be adapted to the native runtime.
    #[error("the example application registration is invalid")]
    ApplicationRegistrationInvalid,
    /// The selected listeners and applications do not form a valid runtime.
    #[error("the server runtime composition is invalid")]
    RuntimeCompositionInvalid,
    /// The composed runtime failed during startup, serving, or shutdown.
    #[error("the server runtime exited with an operational failure")]
    RuntimeExecutionFailed,
}
