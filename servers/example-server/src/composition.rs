// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;
use std::path::Path;

use axum::Router;
use reallyme_server_kit::health::Readiness;
use reallyme_server_kit::http::{HttpListenerName, HttpListenerVisibility};
use reallyme_server_kit::runtime::{HttpServerSpec, ServerRuntime};
use reallyme_server_kit::startup::ServerName;
use reallyme_server_kit::version::BuildInfo;

use crate::cli::config_path_from_args;
use crate::config::ExampleServerConfig;
use crate::error::{ExampleServerError, ExampleServerErrorReason};
use crate::registry::register_compiled_apps;

const SERVER_NAME: &str = "example-server";
const PUBLIC_LISTENER_NAME: &str = "public";

/// Builds the complete native runtime without opening a listener.
///
/// Keeping composition separate from execution makes the compiled app set
/// reviewable and allows CI to validate the server graph without binding a
/// network port.
pub fn build_runtime(config_path: &Path) -> Result<ServerRuntime, ExampleServerError> {
    let config = ExampleServerConfig::load(config_path)?;
    let server_name = ServerName::new(SERVER_NAME)
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::CompiledIdentityInvalid))?;
    let listener_name = HttpListenerName::new(PUBLIC_LISTENER_NAME)
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::CompiledIdentityInvalid))?;
    let http_server = HttpServerSpec::with_listener(
        listener_name,
        HttpListenerVisibility::Public,
        config.http().clone(),
        Router::new(),
    );
    let builder = ServerRuntime::builder()
        .server_name(server_name.clone())
        .observability_config(config.observability().clone())
        .build_info(BuildInfo::from_parts(
            server_name,
            env!("CARGO_PKG_VERSION"),
            option_env!("GIT_SHA"),
            option_env!("BUILD_TIMESTAMP"),
            option_env!("BUILD_PROFILE"),
            option_env!("RUSTC_VERSION"),
        ))
        .readiness(Readiness::new())
        .shutdown_timeout(config.shutdown_timeout())
        .http_server(http_server);
    let builder = register_compiled_apps(builder, config.application_profile())?;

    builder
        .build()
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::RuntimeCompositionInvalid))
}

/// Runs the example server until Ctrl-C or an operating-system shutdown signal.
pub async fn run() -> Result<(), ExampleServerError> {
    let config_path = config_path_from_args(env::args_os())?;
    build_runtime(config_path.as_path())?
        .run()
        .await
        .map_err(|_| ExampleServerError::new(ExampleServerErrorReason::RuntimeExecutionFailed))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::build_runtime;

    #[test]
    fn reference_server_composition_is_valid() {
        assert!(build_runtime(config_path().as_path()).is_ok());
    }

    fn config_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../configs/example-server.jsonc")
    }
}
