// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Connect RPC adapter conventions.

mod conventions;

pub use conventions::{
    ConnectAdapterConventions, ConnectAuthMetadataPolicy, ConnectCodeGenerationWorkflow,
    ConnectDeadlineMetadataPolicy, ConnectErrorMappingPolicy, ConnectFrameworkPolicy,
    ConnectIdempotencyMetadataPolicy, ConnectJsonCompatibilityPolicy, ConnectProtocolEncoding,
    ConnectPublicEncodingPolicy, ConnectRequestEnvelopePolicy, ConnectRuntimeIntegrationPolicy,
    ConnectSchemaSource, ConnectServiceTerminologyPolicy, ConnectVersioningPolicy,
    ConnectWebCompatibilityPolicy,
};
