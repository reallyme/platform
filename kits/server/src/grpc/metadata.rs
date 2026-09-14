// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use tonic::metadata::{Ascii, MetadataMap, MetadataValue};
use tonic::{Request, Status};
use uuid::Uuid;

use super::error::{GrpcMetadataError, GrpcMetadataErrorReason, GrpcMetadataField};
use crate::transport::{IdentifierValueError, RequestId, TraceId};

/// Standard gRPC metadata key used for request identifiers.
pub const GRPC_REQUEST_ID_METADATA_KEY: &str = "x-request-id";
/// Standard gRPC metadata key used for trace identifiers.
pub const GRPC_TRACE_ID_METADATA_KEY: &str = "x-trace-id";

/// Stable correlation IDs attached to a gRPC request.
///
/// The server kit deliberately replaces malformed or missing inbound
/// correlation IDs rather than rejecting the request. This keeps correlation
/// available for public-facing APIs without trusting caller-provided IDs for
/// any security-sensitive decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrpcCorrelationIds {
    request_id: RequestId,
    trace_id: TraceId,
}

impl GrpcCorrelationIds {
    /// Creates a typed correlation-ID bundle.
    pub fn new(request_id: RequestId, trace_id: TraceId) -> Self {
        Self {
            request_id,
            trace_id,
        }
    }

    /// Returns the propagated request identifier.
    pub fn request_id(self) -> RequestId {
        self.request_id
    }

    /// Returns the propagated trace identifier.
    pub fn trace_id(self) -> TraceId {
        self.trace_id
    }
}

/// Returns a validated request identifier from gRPC metadata.
pub fn request_id_from_metadata(
    metadata: &MetadataMap,
) -> Result<Option<RequestId>, GrpcMetadataError> {
    metadata
        .get(GRPC_REQUEST_ID_METADATA_KEY)
        .map(parse_request_id_metadata)
        .transpose()
}

/// Returns a validated trace identifier from gRPC metadata.
pub fn trace_id_from_metadata(
    metadata: &MetadataMap,
) -> Result<Option<TraceId>, GrpcMetadataError> {
    metadata
        .get(GRPC_TRACE_ID_METADATA_KEY)
        .map(parse_trace_id_metadata)
        .transpose()
}

/// Returns request correlation IDs previously attached to request extensions.
pub fn correlation_ids_from_request<T>(request: &Request<T>) -> Option<GrpcCorrelationIds> {
    request.extensions().get::<GrpcCorrelationIds>().copied()
}

/// Returns the propagated request identifier from request extensions.
pub fn request_id_from_request<T>(request: &Request<T>) -> Option<RequestId> {
    correlation_ids_from_request(request).map(GrpcCorrelationIds::request_id)
}

/// Returns the propagated trace identifier from request extensions.
pub fn trace_id_from_request<T>(request: &Request<T>) -> Option<TraceId> {
    correlation_ids_from_request(request).map(GrpcCorrelationIds::trace_id)
}

/// Ensures that the request carries valid request and trace identifiers.
///
/// Missing or malformed inbound identifiers are replaced with freshly
/// generated values, and the normalized values are written back to metadata and
/// request extensions for downstream handlers.
pub fn attach_correlation_ids<T>(
    request: &mut Request<T>,
) -> Result<GrpcCorrelationIds, GrpcMetadataError> {
    let request_id = resolve_request_id(request.metadata());
    let trace_id = resolve_trace_id(request.metadata());

    insert_request_id(request.metadata_mut(), request_id)?;
    insert_trace_id(request.metadata_mut(), trace_id)?;

    let correlation_ids = GrpcCorrelationIds::new(request_id, trace_id);
    request.extensions_mut().insert(correlation_ids);

    Ok(correlation_ids)
}

/// Attaches normalized correlation IDs to a gRPC status.
pub fn attach_correlation_ids_to_status(
    mut status: Status,
    correlation_ids: GrpcCorrelationIds,
) -> Result<Status, GrpcMetadataError> {
    insert_request_id(status.metadata_mut(), correlation_ids.request_id())?;
    insert_trace_id(status.metadata_mut(), correlation_ids.trace_id())?;
    Ok(status)
}

fn resolve_request_id(metadata: &MetadataMap) -> RequestId {
    match request_id_from_metadata(metadata) {
        Ok(Some(request_id)) => request_id,
        Ok(None) | Err(_) => RequestId::generate(),
    }
}

fn resolve_trace_id(metadata: &MetadataMap) -> TraceId {
    match trace_id_from_metadata(metadata) {
        Ok(Some(trace_id)) => trace_id,
        Ok(None) | Err(_) => TraceId::generate(),
    }
}

fn parse_request_id_metadata(value: &MetadataValue<Ascii>) -> Result<RequestId, GrpcMetadataError> {
    let value = value
        .to_str()
        .map_err(|_| GrpcMetadataError::InvalidMetadata {
            field: GrpcMetadataField::RequestId,
            reason: GrpcMetadataErrorReason::InvalidAscii,
        })?;

    RequestId::parse_str(value).map_err(map_request_identifier_error)
}

fn parse_trace_id_metadata(value: &MetadataValue<Ascii>) -> Result<TraceId, GrpcMetadataError> {
    let value = value
        .to_str()
        .map_err(|_| GrpcMetadataError::InvalidMetadata {
            field: GrpcMetadataField::TraceId,
            reason: GrpcMetadataErrorReason::InvalidAscii,
        })?;

    TraceId::parse_str(value).map_err(map_trace_identifier_error)
}

fn insert_request_id(
    metadata: &mut MetadataMap,
    request_id: RequestId,
) -> Result<(), GrpcMetadataError> {
    let value = request_id_metadata_value(request_id)?;
    metadata.insert(GRPC_REQUEST_ID_METADATA_KEY, value);
    Ok(())
}

fn insert_trace_id(metadata: &mut MetadataMap, trace_id: TraceId) -> Result<(), GrpcMetadataError> {
    let value = trace_id_metadata_value(trace_id)?;
    metadata.insert(GRPC_TRACE_ID_METADATA_KEY, value);
    Ok(())
}

fn request_id_metadata_value(
    request_id: RequestId,
) -> Result<MetadataValue<Ascii>, GrpcMetadataError> {
    uuid_metadata_value(request_id.into_uuid()).map_err(|_| GrpcMetadataError::InvalidMetadata {
        field: GrpcMetadataField::RequestId,
        reason: GrpcMetadataErrorReason::InvalidMetadataValue,
    })
}

fn trace_id_metadata_value(trace_id: TraceId) -> Result<MetadataValue<Ascii>, GrpcMetadataError> {
    uuid_metadata_value(trace_id.into_uuid()).map_err(|_| GrpcMetadataError::InvalidMetadata {
        field: GrpcMetadataField::TraceId,
        reason: GrpcMetadataErrorReason::InvalidMetadataValue,
    })
}

fn uuid_metadata_value(
    value: Uuid,
) -> Result<MetadataValue<Ascii>, tonic::metadata::errors::InvalidMetadataValue> {
    let mut buffer = Uuid::encode_buffer();
    let encoded: &str = value.hyphenated().encode_lower(&mut buffer);
    MetadataValue::try_from(encoded)
}

fn map_request_identifier_error(error: IdentifierValueError) -> GrpcMetadataError {
    let reason = match error {
        IdentifierValueError::InvalidUuid => GrpcMetadataErrorReason::InvalidUuid,
    };

    GrpcMetadataError::InvalidMetadata {
        field: GrpcMetadataField::RequestId,
        reason,
    }
}

fn map_trace_identifier_error(error: IdentifierValueError) -> GrpcMetadataError {
    let reason = match error {
        IdentifierValueError::InvalidUuid => GrpcMetadataErrorReason::InvalidUuid,
    };

    GrpcMetadataError::InvalidMetadata {
        field: GrpcMetadataField::TraceId,
        reason,
    }
}

#[cfg(test)]
#[path = "metadata_tests.rs"]
mod tests;
