// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! In-process typed port adapter for the example app.

use reallyme_example_contract::{
    ExampleContractError, ExamplePort, ExamplePortCall, HelloRequest, HelloResponse,
};

use crate::app::ExampleAppContext;

/// In-memory adapter used when the server hosts this app and a caller in the
/// same process binds to the example contract.
#[derive(Debug, Clone)]
pub struct InProcessExamplePort {
    context: ExampleAppContext,
}

impl InProcessExamplePort {
    /// Constructs an in-process example port adapter.
    pub const fn new(context: ExampleAppContext) -> Self {
        Self { context }
    }
}

impl ExamplePort for InProcessExamplePort {
    fn hello(&self, request: HelloRequest) -> ExamplePortCall<'_, HelloResponse> {
        Box::pin(async move {
            crate::app::hello(&self.context, request, None)
                .map_err(map_example_app_error_to_contract_error)
        })
    }
}

fn map_example_app_error_to_contract_error(
    error: crate::app::ExampleAppError,
) -> ExampleContractError {
    match error {
        crate::app::ExampleAppError::HelloDisabled => ExampleContractError::PermissionDenied,
        crate::app::ExampleAppError::MetricConfigurationInvalid => {
            ExampleContractError::Unavailable
        }
    }
}

#[cfg(test)]
mod tests {
    use reallyme_example_contract::{ExamplePort, HelloRequest};

    use super::InProcessExamplePort;
    use crate::app::for_tests_only_local_context;

    #[tokio::test]
    async fn in_process_port_calls_same_app_core() {
        let port = InProcessExamplePort::new(for_tests_only_local_context());
        let response = port
            .hello(HelloRequest)
            .await
            .expect("example app should serve");

        assert_eq!(response.body(), "hello from example-app");
    }
}
