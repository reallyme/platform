// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use example_app::adapters::server::runtime_app_from_config_document;
use example_app::app::example_app_config_document;
use example_app::ports::ExamplePorts;
use reallyme_app_kit::AppConfigProfile;
use reallyme_server_kit::runtime::ServerRuntimeBuilder;

use crate::error::{ExampleServerError, ExampleServerErrorReason};

/// Registers every application compiled into this server artifact.
///
/// The registry is deliberately code, not a plugin loader. Configuration may
/// choose a profile for an application in this compiled set, but it cannot
/// introduce application code that was absent when the binary was built.
pub(crate) fn register_compiled_apps(
    builder: ServerRuntimeBuilder,
    application_profile: AppConfigProfile,
) -> Result<ServerRuntimeBuilder, ExampleServerError> {
    let document = example_app_config_document(application_profile)
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::ApplicationConfigInvalid))?;
    let app = runtime_app_from_config_document(&document, ExamplePorts::unconfigured()).map_err(
        |_| ExampleServerError::new(ExampleServerErrorReason::ApplicationRegistrationInvalid),
    )?;

    Ok(builder.app(app))
}
