// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use connectrpc::{RequestContext, Response, ServiceRequest, ServiceResult};
use reallyme_example_contract::generated::connect::reallyme::example::v1::ExampleService;
use reallyme_example_contract::generated::proto::reallyme::example::v1::HelloResponse;

use crate::app::{ExampleAppContext, HelloRequest};

use super::error::map_example_app_error_to_connect_error;

/// Connect RPC service adapter for the example app.
///
/// This type translates generated transport request/response types into the
/// host-neutral app use-case. It deliberately does not own listeners,
/// middleware, readiness, shutdown, or process lifecycle.
#[derive(Debug)]
pub struct ExampleConnectService {
    context: ExampleAppContext,
}

impl ExampleConnectService {
    /// Constructs the Connect service adapter.
    pub const fn new(context: ExampleAppContext) -> Self {
        Self { context }
    }
}

// connectrpc codegen currently uses impl Trait in the generated service trait, while this
// implementation intentionally narrows the return type to keep the adapter allocation-light.
#[allow(refining_impl_trait)]
impl ExampleService for ExampleConnectService {
    async fn hello(
        &self,
        ctx: RequestContext,
        request: ServiceRequest<
            '_,
            reallyme_example_contract::generated::proto::reallyme::example::v1::HelloRequest,
        >,
    ) -> ServiceResult<HelloResponse> {
        // The generated hello RPC payload is intentionally unit-shaped.
        let _ = request;
        let deadline = ctx
            .deadline()
            .and_then(|deadline| deadline.checked_duration_since(std::time::Instant::now()));
        let response = crate::app::hello(&self.context, HelloRequest, deadline)
            .map_err(map_example_app_error_to_connect_error)?;

        Response::ok(HelloResponse {
            // String allocation here is intrinsic to the wire encoding of the
            // generated proto response string field.
            message: response.body().to_owned(),
            ..Default::default()
        })
    }
}
