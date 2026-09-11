// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed errors for the Hephaestus agent.

use thiserror::Error;

/// Stable, non-secret error reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HephaestusAgentErrorReason {
    /// Configuration was missing or malformed.
    InvalidConfig,
    /// A configured URL was invalid or unsupported.
    InvalidUrl,
    /// A configured filesystem path was invalid or outside the allowed shape.
    InvalidPath,
    /// A configured identity value failed validation.
    InvalidIdentity,
    /// A configured duration or numeric value was invalid.
    InvalidNumber,
    /// A configured header value was invalid.
    InvalidHeader,
    /// A local file read failed.
    FileReadFailed,
    /// A local file write failed.
    FileWriteFailed,
    /// A local file delete failed.
    FileDeleteFailed,
    /// A local command failed to start, timed out, or returned a bad exit code.
    CommandFailed,
    /// A requested local executable was not installed at its expected path.
    CommandUnavailable,
    /// A local command returned output that could not be parsed safely.
    CommandOutputInvalid,
    /// A cAdvisor request failed or returned an invalid payload.
    CadvisorUnavailable,
    /// A local Prometheus metrics endpoint failed or returned an invalid payload.
    MetricsEndpointUnavailable,
    /// A Connect RPC call to Hephaestus failed.
    ConnectFailed,
    /// Central Hephaestus rejected agent registration.
    RegistrationRejected,
    /// A bootstrap token was unavailable during first registration.
    BootstrapTokenUnavailable,
    /// A runtime token was unavailable after registration.
    RuntimeTokenUnavailable,
    /// A node identity key could not be generated, read, or validated.
    IdentityKeyUnavailable,
    /// A cryptographic digest or encoding operation failed.
    CryptoUnavailable,
    /// A TLS client could not be constructed.
    TlsUnavailable,
    /// A domain DTO rejected locally observed state.
    DomainValidationFailed,
}

/// Agent error with a stable typed reason.
#[derive(Debug, Error)]
#[error("hephaestus agent error: {reason:?}")]
pub struct HephaestusAgentError {
    reason: HephaestusAgentErrorReason,
}

impl HephaestusAgentError {
    /// Constructs an error from a stable reason.
    pub const fn new(reason: HephaestusAgentErrorReason) -> Self {
        Self { reason }
    }

    /// Returns the stable reason.
    pub const fn reason(&self) -> HephaestusAgentErrorReason {
        self.reason
    }
}

/// Convenient result alias for agent code.
pub type AgentResult<T> = Result<T, HephaestusAgentError>;
