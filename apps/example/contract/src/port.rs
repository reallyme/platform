// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::pin::Pin;

use reallyme_app_kit::{AppDownstreamPortDescriptor, AppPortDescriptor, AppPortName};

use crate::{ExampleContractError, HelloRequest, HelloResponse};

/// Object-safe future returned by example app ports.
pub type ExamplePortCall<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ExampleContractError>> + Send + 'a>>;

/// Host-neutral example app port.
pub trait ExamplePort: Send + Sync {
    /// Calls the example hello use-case.
    fn hello(&self, request: HelloRequest) -> ExamplePortCall<'_, HelloResponse>;
}

/// Returns the standard in-process port descriptor for this contract.
pub fn example_port_descriptor()
-> Result<AppDownstreamPortDescriptor, reallyme_app_kit::AppKitError> {
    Ok(AppDownstreamPortDescriptor::new(AppPortDescriptor::new(
        AppPortName::new("example")?,
        true,
    )))
}

/// Zero-sized marker for in-process example-port adapters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InProcessExamplePortDescriptor;
