// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use thiserror::Error;

/// Valkey configuration field identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyConfigField {
    /// Prefix used to derive Valkey environment-variable names.
    EnvironmentPrefix,
    /// Server hostname or IP address.
    Host,
    /// Server TCP port.
    Port,
    /// Logical database number.
    Database,
    /// Key namespace prefix.
    KeyPrefix,
    /// Optional ACL username.
    Username,
    /// Optional password or access token.
    Password,
    /// Connection establishment deadline.
    ConnectionTimeout,
    /// Command response deadline.
    ResponseTimeout,
    /// Automatic reconnect attempt count.
    RetryAttempts,
    /// Concurrent in-flight command ceiling.
    ConcurrencyLimit,
    /// Outbound command queue bound.
    PipelineBufferSize,
    /// Transport security mode.
    TransportSecurity,
    /// Private TLS CA certificate path.
    TlsCaCertificatePath,
}

/// Stable Valkey configuration failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyConfigErrorReason {
    /// A required field was empty.
    Empty,
    /// A field exceeded its fixed bound.
    TooLarge,
    /// A field did not match its required syntax.
    InvalidSyntax,
    /// A numeric field was zero.
    MustBePositive,
    /// An environment value was not valid Unicode.
    InvalidEncoding,
    /// A value conflicts with another explicitly selected option.
    Incompatible,
}

/// Stable Valkey connection setup failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeySetupErrorReason {
    /// The configured endpoint could not be represented by the client.
    InvalidEndpoint,
    /// TLS provider initialization was unavailable.
    TlsProviderUnavailable,
    /// The initial connection or authentication handshake failed.
    ConnectionUnavailable,
    /// The server rejected configured authentication credentials.
    AuthenticationRejected,
    /// A custom TLS trust file could not be loaded.
    TlsTrustUnavailable,
    /// A custom TLS trust file was malformed or contained no certificates.
    TlsTrustInvalid,
    /// A custom TLS trust file exceeded the bounded startup policy.
    TlsTrustTooLarge,
}

/// Stable Valkey command failure reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyCommandErrorReason {
    /// The connection was unavailable.
    ConnectionUnavailable,
    /// A command exceeded its response deadline.
    Timeout,
    /// The server rejected a command or its arguments.
    Rejected,
    /// The response could not be decoded into the required type.
    InvalidResponse,
}

/// Conservative retry guidance for an app-owned Valkey operation.
///
/// The hint does not prove that an operation is idempotent. Applications must
/// still enforce a bounded deadline and decide whether replaying their complete
/// operation is safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyRetryHint {
    /// Do not retry automatically.
    DoNotRetry,
    /// Retry only when the complete operation is idempotent.
    RetryIdempotentOperation,
    /// Retry an idempotent operation after bounded randomized backoff.
    RetryIdempotentOperationAfterBackoff,
}

impl ValkeyCommandErrorReason {
    /// Returns conservative retry guidance for this failure category.
    pub const fn retry_hint(self) -> ValkeyRetryHint {
        match self {
            Self::ConnectionUnavailable => ValkeyRetryHint::RetryIdempotentOperationAfterBackoff,
            Self::Timeout => ValkeyRetryHint::RetryIdempotentOperation,
            Self::Rejected | Self::InvalidResponse => ValkeyRetryHint::DoNotRetry,
        }
    }
}

/// Bounded client-side data kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyDataKind {
    /// Binary key suffix.
    Key,
    /// Binary value.
    Value,
    /// Expiration interval.
    TimeToLive,
}

/// Stable client-side data validation reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyDataErrorReason {
    /// A required value was empty.
    Empty,
    /// A bounded value exceeded its maximum size.
    TooLarge,
    /// A numeric value was outside its accepted range.
    OutOfRange,
}

/// Typed Valkey kit error without server text, keys, values, or credentials.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ValkeyError {
    /// Static configuration validation failed.
    #[error("valkey configuration is invalid")]
    Config {
        /// Field that failed validation.
        field: ValkeyConfigField,
        /// Stable validation reason.
        reason: ValkeyConfigErrorReason,
    },
    /// Initial client or connection setup failed.
    #[error("valkey client setup failed")]
    Setup {
        /// Stable setup reason.
        reason: ValkeySetupErrorReason,
    },
    /// A command failed.
    #[error("valkey command failed")]
    Command {
        /// Stable command failure reason.
        reason: ValkeyCommandErrorReason,
    },
    /// A key, value, or TTL failed local validation.
    #[error("valkey operation data is invalid")]
    InvalidData {
        /// Kind of value that failed validation.
        kind: ValkeyDataKind,
        /// Stable validation reason.
        reason: ValkeyDataErrorReason,
    },
}

/// Result alias for Valkey kit operations.
pub type ValkeyResult<T> = Result<T, ValkeyError>;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
