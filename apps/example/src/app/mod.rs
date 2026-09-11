// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Host-neutral app orchestration layer for the example app.

mod config;
mod config_document;
mod context;
mod descriptor;
mod error;
mod health;
mod hello;

pub use config::{ExampleAppConfig, ExampleConfigError};
pub use config_document::{
    ExampleAppConfigDocument, ExampleCustomConfig, example_app_config_document,
    parse_example_app_config_document,
};
#[cfg(any(test, feature = "testing"))]
pub use context::for_tests_only_local_context;
pub use context::{
    ExampleAppContext, ExampleAppCore, ExampleAppState, context_from_config_document, new_context,
};
pub use descriptor::{
    EXAMPLE_APP_NAME, EXAMPLE_BACKGROUND_TASK_NAME, EXAMPLE_CLEANUP_HOOK_NAME,
    EXAMPLE_STARTUP_CHECK_NAME, example_app_descriptor, example_app_metadata,
};
pub use error::{ExampleAppError, ExampleAppErrorKind};
pub use health::app_health;
pub use hello::{HelloRequest, HelloResponse, hello};
