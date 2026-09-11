// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use connectrpc::Router;
use reallyme_example_contract::generated::connect::reallyme::example::v1::ExampleServiceExt;

use crate::app::ExampleAppContext;

use super::service::ExampleConnectService;

/// Builds the app-owned Connect router.
///
/// The example app carries a real Buf/protobuf contract, but generated Connect
/// handlers remain transport adapters only. The app core stays host-neutral and
/// the native server host owns the process lifecycle.
pub fn connect_router(context: ExampleAppContext) -> Router {
    Arc::new(ExampleConnectService::new(context)).register(Router::new())
}

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use buffa::Message;
    use bytes::Bytes;
    use reallyme_app_kit::ConnectCodeGenerationWorkflow;
    use reallyme_example_contract::generated::proto::reallyme::example::v1::{
        HelloRequest, HelloResponse,
    };

    use super::connect_router;
    use crate::adapters::connect::{ConnectAdapterConventions, EXAMPLE_HELLO_CONNECT_RPC_PATH};
    use crate::app::for_tests_only_local_context;

    #[tokio::test]
    async fn binary_connect_route_calls_host_neutral_app_core() {
        let server =
            TestServer::new(connect_router(for_tests_only_local_context()).into_axum_router());

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
}
