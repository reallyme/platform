// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

#[tokio::main]
async fn main() -> Result<(), example_server::ExampleServerError> {
    example_server::run().await
}
