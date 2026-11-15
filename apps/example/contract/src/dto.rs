// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Host-neutral hello request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HelloRequest;

/// Host-neutral hello response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HelloResponse {
    body: &'static str,
}

impl HelloResponse {
    /// Constructs a hello response.
    pub const fn new(body: &'static str) -> Self {
        Self { body }
    }

    /// Returns the response body.
    pub const fn body(self) -> &'static str {
        self.body
    }
}
