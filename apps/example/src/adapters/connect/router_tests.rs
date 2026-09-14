// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use axum_test::TestServer;
use buffa::Message;
use bytes::Bytes;
use reallyme_app_kit::ConnectCodeGenerationWorkflow;
use reallyme_example_contract::generated::proto::reallyme::example::v1::{
    HelloRequest, HelloResponse,
};

use super::connect_router;
use crate::adapters::connect::{ConnectAdapterConventions, EXAMPLE_HELLO_CONNECT_RPC_PATH};
use crate::app::{ExampleAppConfig, new_context};
use crate::ports::ExamplePorts;

fn local_context() -> crate::app::ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

#[tokio::test]
async fn binary_connect_route_calls_host_neutral_app_core() {
    let server = TestServer::new(connect_router(local_context()).into_axum_router());

    let response = server
        .post(EXAMPLE_HELLO_CONNECT_RPC_PATH)
        .bytes(Bytes::from(HelloRequest::default().encode_to_vec()))
        .add_header("content-type", "application/proto")
        .await;

    response.assert_status_ok();

    let response = HelloResponse::decode(&mut response.as_bytes().as_ref())
        .expect("binary Connect hello response should decode");
    assert_eq!(response.message, "hello from example-app");
}

#[test]
fn example_connect_adapter_uses_buf_generate_workflow() {
    assert_eq!(
        ConnectAdapterConventions.code_generation_workflow(),
        ConnectCodeGenerationWorkflow::BufGenerateBeforeRustChecks,
    );
}
