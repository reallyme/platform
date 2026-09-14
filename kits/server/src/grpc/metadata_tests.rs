// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use tonic::Request;

use super::{
    GRPC_REQUEST_ID_METADATA_KEY, GRPC_TRACE_ID_METADATA_KEY, GrpcCorrelationIds,
    attach_correlation_ids, attach_correlation_ids_to_status, correlation_ids_from_request,
    request_id_from_metadata, request_id_from_request, trace_id_from_metadata,
    trace_id_from_request,
};
use crate::grpc::{GrpcMetadataError, GrpcMetadataErrorReason, GrpcMetadataField};
use crate::transport::{RequestId, TraceId};

#[test]
fn metadata_helpers_parse_valid_values() {
    let request_id = RequestId::generate();
    let trace_id = TraceId::generate();
    let mut request = Request::new(());

    request.metadata_mut().insert(
        GRPC_REQUEST_ID_METADATA_KEY,
        request_id
            .into_uuid()
            .to_string()
            .parse()
            .expect("generated request ID should produce metadata"),
    );
    request.metadata_mut().insert(
        GRPC_TRACE_ID_METADATA_KEY,
        trace_id
            .into_uuid()
            .to_string()
            .parse()
            .expect("generated trace ID should produce metadata"),
    );

    assert_eq!(
        request_id_from_metadata(request.metadata()),
        Ok(Some(request_id))
    );
    assert_eq!(
        trace_id_from_metadata(request.metadata()),
        Ok(Some(trace_id))
    );
}

#[test]
fn metadata_helpers_reject_invalid_values() {
    let mut request = Request::new(());
    request.metadata_mut().insert(
        GRPC_REQUEST_ID_METADATA_KEY,
        "not-a-uuid"
            .parse()
            .expect("static invalid UUID should still be metadata"),
    );

    let result = request_id_from_metadata(request.metadata());

    assert_eq!(
        result,
        Err(GrpcMetadataError::InvalidMetadata {
            field: GrpcMetadataField::RequestId,
            reason: GrpcMetadataErrorReason::InvalidUuid,
        })
    );
}

#[test]
fn attach_correlation_ids_replaces_missing_or_invalid_values() {
    let mut request = Request::new(());
    request.metadata_mut().insert(
        GRPC_TRACE_ID_METADATA_KEY,
        "not-a-uuid"
            .parse()
            .expect("static invalid UUID should still be metadata"),
    );

    let correlation_ids = attach_correlation_ids(&mut request)
        .expect("correlation IDs should be attached successfully");

    assert_eq!(
        request_id_from_request(&request),
        Some(correlation_ids.request_id())
    );
    assert_eq!(
        trace_id_from_request(&request),
        Some(correlation_ids.trace_id())
    );
    assert_eq!(
        correlation_ids_from_request(&request),
        Some(correlation_ids)
    );
    assert_eq!(
        request_id_from_metadata(request.metadata()),
        Ok(Some(correlation_ids.request_id()))
    );
    assert_eq!(
        trace_id_from_metadata(request.metadata()),
        Ok(Some(correlation_ids.trace_id()))
    );
}

#[test]
fn correlation_ids_can_be_attached_to_status_metadata() {
    let correlation_ids = GrpcCorrelationIds::new(RequestId::generate(), TraceId::generate());
    let status =
        attach_correlation_ids_to_status(tonic::Status::internal("failure"), correlation_ids)
            .expect("generated correlation IDs should attach cleanly");

    assert_eq!(
        request_id_from_metadata(status.metadata()),
        Ok(Some(correlation_ids.request_id()))
    );
    assert_eq!(
        trace_id_from_metadata(status.metadata()),
        Ok(Some(correlation_ids.trace_id()))
    );
}
