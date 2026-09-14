// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{
    EXAMPLE_HELLO_CONNECT_RPC_PATH, EXAMPLE_HELLO_RPC, EXAMPLE_HELLO_RPC_METHOD_NAME,
    EXAMPLE_HELLO_RPC_PATH, EXAMPLE_RPC_SERVICE_NAME,
};

#[test]
fn example_rpc_path_is_feature_neutral() {
    assert_eq!(
        EXAMPLE_RPC_SERVICE_NAME,
        "reallyme.example.v1.ExampleService",
    );
    assert_eq!(EXAMPLE_HELLO_RPC_METHOD_NAME, "Hello");
    assert_eq!(
        EXAMPLE_HELLO_RPC_PATH,
        "/reallyme.example.v1.ExampleService/Hello",
    );
    assert_eq!(EXAMPLE_HELLO_CONNECT_RPC_PATH, EXAMPLE_HELLO_RPC_PATH);
    assert_eq!(EXAMPLE_HELLO_RPC.service_name(), EXAMPLE_RPC_SERVICE_NAME);
    assert_eq!(
        EXAMPLE_HELLO_RPC.method_name(),
        EXAMPLE_HELLO_RPC_METHOD_NAME,
    );
    assert_eq!(EXAMPLE_HELLO_RPC.path(), EXAMPLE_HELLO_RPC_PATH);
}
