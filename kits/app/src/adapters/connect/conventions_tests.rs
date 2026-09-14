// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    ConnectAdapterConventions, ConnectAuthMetadataPolicy, ConnectCodeGenerationWorkflow,
    ConnectDeadlineMetadataPolicy, ConnectErrorMappingPolicy, ConnectFrameworkPolicy,
    ConnectIdempotencyMetadataPolicy, ConnectJsonCompatibilityPolicy, ConnectProtocolEncoding,
    ConnectPublicEncodingPolicy, ConnectRequestEnvelopePolicy, ConnectRuntimeIntegrationPolicy,
    ConnectSchemaSource, ConnectServiceTerminologyPolicy, ConnectVersioningPolicy,
    ConnectWebCompatibilityPolicy,
};

#[test]
fn connect_is_buf_protobuf_binary_first() {
    assert_eq!(
        ConnectAdapterConventions.schema_source(),
        ConnectSchemaSource::BufProtobuf
    );
    assert_eq!(
        ConnectAdapterConventions.service_terminology(),
        ConnectServiceTerminologyPolicy::ProtoServiceIsAppOwnedRpcSurface
    );
    assert_eq!(
        ConnectAdapterConventions.code_generation_workflow(),
        ConnectCodeGenerationWorkflow::BufGenerateBeforeRustChecks
    );
    assert_eq!(
        ConnectAdapterConventions.runtime_integration(),
        ConnectRuntimeIntegrationPolicy::TowerNativeRouter
    );
    assert_eq!(
        ConnectAdapterConventions.framework_policy(),
        ConnectFrameworkPolicy::FrameworkAgnosticHandlers
    );
    assert_eq!(
        ConnectAdapterConventions.default_encoding(),
        ConnectProtocolEncoding::ProtobufBinary
    );
    assert_eq!(
        ConnectAdapterConventions.public_encoding_policy(),
        ConnectPublicEncodingPolicy::BinaryProtobufFirst
    );
    assert_eq!(
        ConnectAdapterConventions.json_compatibility_policy(),
        ConnectJsonCompatibilityPolicy::CompatibilityOnlyByException
    );
    assert!(!ConnectAdapterConventions.http_json_is_canonical());
}

#[test]
fn connect_adapter_conventions_keep_runtime_concerns_out_of_apps() {
    assert!(!ConnectAdapterConventions.app_starts_listener());
    assert_eq!(
        ConnectAdapterConventions.deadline_metadata(),
        ConnectDeadlineMetadataPolicy::RespectCallerDeadlineWithinHostMaximum
    );
    assert_eq!(
        ConnectAdapterConventions.auth_metadata(),
        ConnectAuthMetadataPolicy::ExtractAndRedact
    );
}

#[test]
fn connect_adapter_conventions_preserve_stable_contract_rules() {
    assert_eq!(
        ConnectAdapterConventions.error_mapping(),
        ConnectErrorMappingPolicy::TypedPublicContractOnly
    );
    assert_eq!(
        ConnectAdapterConventions.request_envelope(),
        ConnectRequestEnvelopePolicy::ProtoMessagesAreCanonical
    );
    assert_eq!(
        ConnectAdapterConventions.idempotency_metadata(),
        ConnectIdempotencyMetadataPolicy::ReviewedPerMethod
    );
    assert_eq!(
        ConnectAdapterConventions.versioning(),
        ConnectVersioningPolicy::VersionedProtoPackages
    );
    assert_eq!(
        ConnectAdapterConventions.connect_web_compatibility(),
        ConnectWebCompatibilityPolicy::SameProtoContract
    );
}
