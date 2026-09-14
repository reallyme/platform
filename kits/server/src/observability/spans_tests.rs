// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::Method;

use super::{background_task_span, grpc_request_span, http_request_span};
use crate::startup::TaskName;
use crate::transport::{RequestId, TraceId};

#[test]
fn span_names_remain_stable() {
    let task_name = TaskName::new("worker").expect("valid task name");
    let request_id = RequestId::generate();
    let trace_id = TraceId::generate();

    assert_eq!(
        background_task_span(&task_name)
            .metadata()
            .expect("span metadata should exist")
            .name(),
        "background_task"
    );
    assert_eq!(
        http_request_span(
            &Method::GET,
            Some("/readyz"),
            Some(request_id),
            Some(trace_id)
        )
        .metadata()
        .expect("span metadata should exist")
        .name(),
        "http_request"
    );
    assert_eq!(
        grpc_request_span(
            "reallyme.api.UserService",
            "GetUser",
            Some(request_id),
            Some(trace_id)
        )
        .metadata()
        .expect("span metadata should exist")
        .name(),
        "grpc_request"
    );
}
