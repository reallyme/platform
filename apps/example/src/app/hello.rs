// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::record_app_metric_counter_by_name;
pub use reallyme_example_contract::{HelloRequest, HelloResponse};

use super::{ExampleAppContext, ExampleAppError};

const EXAMPLE_METRIC_NAMESPACE: &str = "reallyme_example";
const HELLO_REQUESTS_METRIC: &str = "hello_requests";
const HELLO_RESPONSE_BODY: &str = "hello from example-app";

/// Runs the example hello use-case through the shared app context.
pub fn hello(
    context: &ExampleAppContext,
    request: HelloRequest,
    _deadline: Option<std::time::Duration>,
) -> Result<HelloResponse, ExampleAppError> {
    // Deadline is currently only represented in the function signature so
    // call sites can pass caller intent through consistently. Real work should
    // enforce it with a timeout around the service call path.
    // This use-case currently has no request fields with behavioral impact.
    let _ = request;

    if !context.core().state().config().hello_enabled() {
        return Err(ExampleAppError::HelloDisabled);
    }

    record_hello_request()?;
    Ok(HelloResponse::new(HELLO_RESPONSE_BODY))
}

fn record_hello_request() -> Result<(), ExampleAppError> {
    record_app_metric_counter_by_name(EXAMPLE_METRIC_NAMESPACE, HELLO_REQUESTS_METRIC)
        .map_err(|_error| ExampleAppError::MetricConfigurationInvalid)
}

#[cfg(test)]
#[path = "hello_tests.rs"]
mod tests;
