// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_example_contract::{ExamplePort, HelloRequest};

use super::InProcessExamplePort;
use crate::app::{ExampleAppConfig, new_context};
use crate::ports::ExamplePorts;

fn local_context() -> crate::app::ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

#[tokio::test]
async fn in_process_port_calls_same_app_core() {
    let port = InProcessExamplePort::new(local_context());
    let response = port
        .hello(HelloRequest)
        .await
        .expect("example app should serve");

    assert_eq!(response.body(), "hello from example-app");
}
