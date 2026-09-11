// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use reallyme_app_kit::{StandardAppContext, StandardAppCore, StandardAppState};

use crate::ports::ExamplePorts;

use super::{ExampleAppConfig, ExampleAppConfigDocument};

/// Host-neutral example app state.
pub type ExampleAppState = StandardAppState<ExampleAppConfig>;

/// Host-neutral example app core.
pub type ExampleAppCore = StandardAppCore<ExampleAppState>;

/// Shared app context used by all example app transport adapters.
pub type ExampleAppContext = StandardAppContext<ExampleAppCore, ExamplePorts>;

/// Constructs app context from validated app config and typed ports.
pub fn new_context(config: ExampleAppConfig, ports: ExamplePorts) -> ExampleAppContext {
    ExampleAppContext::new(ExampleAppCore::new(ExampleAppState::new(config)), ports)
}

/// Constructs a local/example context with deterministic placeholder ports.
#[cfg(any(test, feature = "testing"))]
pub fn for_tests_only_local_context() -> ExampleAppContext {
    new_context(ExampleAppConfig::new(true), ExamplePorts::unconfigured())
}

/// Constructs context from a validated standard app JSONC config document.
pub fn context_from_config_document(
    document: &ExampleAppConfigDocument,
    ports: ExamplePorts,
) -> ExampleAppContext {
    new_context(
        ExampleAppConfig::new(document.custom().hello_enabled()),
        ports,
    )
}
