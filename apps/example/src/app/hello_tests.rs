// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::app::{ExampleAppConfig, ExampleAppError, HelloRequest, hello, new_context};
use crate::ports::ExamplePorts;

#[test]
fn hello_use_case_is_host_neutral() {
    let context = new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured());
    let response = hello(&context, HelloRequest, None).expect("enabled hello should succeed");

    assert_eq!(response.body(), "hello from example-app");
}

#[test]
fn disabled_hello_returns_typed_app_error() {
    let context = new_context(ExampleAppConfig::new(false), ExamplePorts::unconfigured());

    assert_eq!(
        hello(&context, HelloRequest, None),
        Err(ExampleAppError::HelloDisabled),
    );
}
