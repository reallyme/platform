// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Canonical schema source for Connect/gRPC/HTTP adapter contracts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectSchemaSource {
    /// Buf-managed protobuf contracts are canonical.
    #[default]
    BufProtobuf,
}

/// Terminology boundary between protobuf and ReallyMe runtime architecture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectServiceTerminologyPolicy {
    /// A protobuf `service` is an app-owned RPC trait/surface, not a server process.
    #[default]
    ProtoServiceIsAppOwnedRpcSurface,
}

/// Required Rust code-generation workflow for Connect-enabled apps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectCodeGenerationWorkflow {
    /// `buf generate` before Rust formatting and checks is the production workflow.
    #[default]
    BufGenerateBeforeRustChecks,
}

/// Runtime integration posture for Connect adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectRuntimeIntegrationPolicy {
    /// App adapters return Tower-native Connect routers/services.
    #[default]
    TowerNativeRouter,
}

/// Framework coupling posture for app-owned Connect handlers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectFrameworkPolicy {
    /// Connect handlers should remain independent of Axum/Hyper listener ownership.
    #[default]
    FrameworkAgnosticHandlers,
}

/// Default Connect protocol encoding.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectProtocolEncoding {
    /// Binary protobuf request/response bodies are the default.
    #[default]
    ProtobufBinary,
}

/// Public RPC encoding posture for Connect-enabled apps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectPublicEncodingPolicy {
    /// Public RPC clients should prefer binary protobuf over JSON encodings.
    #[default]
    BinaryProtobufFirst,
}

/// JSON compatibility posture for Connect-enabled apps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectJsonCompatibilityPolicy {
    /// JSON is a reviewed compatibility/debug surface, not the default client path.
    #[default]
    CompatibilityOnlyByException,
}

/// Public error mapping posture for Connect adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectErrorMappingPolicy {
    /// App/internal errors must pass through typed public RPC error mapping.
    #[default]
    TypedPublicContractOnly,
}

/// Request/response envelope posture for Connect adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectRequestEnvelopePolicy {
    /// Requests and responses should map directly to protobuf messages.
    #[default]
    ProtoMessagesAreCanonical,
}

/// Deadline/timeout metadata posture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectDeadlineMetadataPolicy {
    /// Caller deadlines are respected within host/server maximums.
    #[default]
    RespectCallerDeadlineWithinHostMaximum,
}

/// Authentication metadata posture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectAuthMetadataPolicy {
    /// Auth metadata is extracted by adapters and must never be logged.
    #[default]
    ExtractAndRedact,
}

/// Idempotency metadata posture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectIdempotencyMetadataPolicy {
    /// Mutating RPCs may define reviewed idempotency metadata.
    #[default]
    ReviewedPerMethod,
}

/// API versioning posture for Connect services.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectVersioningPolicy {
    /// Versioning is encoded in protobuf package names, e.g. `reallyme.api.v1`.
    #[default]
    VersionedProtoPackages,
}

/// Connect-Web compatibility posture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConnectWebCompatibilityPolicy {
    /// Connect-Web compatibility is allowed when it preserves the same protobuf contract.
    #[default]
    SameProtoContract,
}

/// Conventions for app-owned Connect RPC adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ConnectAdapterConventions;

impl ConnectAdapterConventions {
    /// Returns the canonical schema source.
    pub const fn schema_source(self) -> ConnectSchemaSource {
        ConnectSchemaSource::BufProtobuf
    }

    /// Returns the terminology boundary for protobuf `service` declarations.
    pub const fn service_terminology(self) -> ConnectServiceTerminologyPolicy {
        ConnectServiceTerminologyPolicy::ProtoServiceIsAppOwnedRpcSurface
    }

    /// Returns the required Rust code-generation workflow.
    pub const fn code_generation_workflow(self) -> ConnectCodeGenerationWorkflow {
        ConnectCodeGenerationWorkflow::BufGenerateBeforeRustChecks
    }

    /// Returns the runtime integration policy.
    pub const fn runtime_integration(self) -> ConnectRuntimeIntegrationPolicy {
        ConnectRuntimeIntegrationPolicy::TowerNativeRouter
    }

    /// Returns the framework coupling policy.
    pub const fn framework_policy(self) -> ConnectFrameworkPolicy {
        ConnectFrameworkPolicy::FrameworkAgnosticHandlers
    }

    /// Returns the default wire encoding.
    pub const fn default_encoding(self) -> ConnectProtocolEncoding {
        ConnectProtocolEncoding::ProtobufBinary
    }

    /// Returns the public RPC encoding posture.
    pub const fn public_encoding_policy(self) -> ConnectPublicEncodingPolicy {
        ConnectPublicEncodingPolicy::BinaryProtobufFirst
    }

    /// Returns the JSON compatibility posture.
    pub const fn json_compatibility_policy(self) -> ConnectJsonCompatibilityPolicy {
        ConnectJsonCompatibilityPolicy::CompatibilityOnlyByException
    }

    /// Returns the public error mapping policy.
    pub const fn error_mapping(self) -> ConnectErrorMappingPolicy {
        ConnectErrorMappingPolicy::TypedPublicContractOnly
    }

    /// Returns the request/response envelope policy.
    pub const fn request_envelope(self) -> ConnectRequestEnvelopePolicy {
        ConnectRequestEnvelopePolicy::ProtoMessagesAreCanonical
    }

    /// Returns the deadline/timeout metadata policy.
    pub const fn deadline_metadata(self) -> ConnectDeadlineMetadataPolicy {
        ConnectDeadlineMetadataPolicy::RespectCallerDeadlineWithinHostMaximum
    }

    /// Returns the authentication metadata policy.
    pub const fn auth_metadata(self) -> ConnectAuthMetadataPolicy {
        ConnectAuthMetadataPolicy::ExtractAndRedact
    }

    /// Returns the idempotency metadata policy.
    pub const fn idempotency_metadata(self) -> ConnectIdempotencyMetadataPolicy {
        ConnectIdempotencyMetadataPolicy::ReviewedPerMethod
    }

    /// Returns the versioning policy.
    pub const fn versioning(self) -> ConnectVersioningPolicy {
        ConnectVersioningPolicy::VersionedProtoPackages
    }

    /// Returns the Connect-Web compatibility policy.
    pub const fn connect_web_compatibility(self) -> ConnectWebCompatibilityPolicy {
        ConnectWebCompatibilityPolicy::SameProtoContract
    }

    /// Returns whether app Connect adapters should start listeners.
    pub const fn app_starts_listener(self) -> bool {
        false
    }

    /// Returns whether HTTP/JSON is the canonical app contract.
    pub const fn http_json_is_canonical(self) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "conventions_tests.rs"]
mod tests;
