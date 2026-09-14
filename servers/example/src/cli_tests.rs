// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::path::Path;

use super::config_path_from_args;
use crate::error::ExampleServerErrorReason;

#[test]
fn config_flag_selects_server_document() -> Result<(), crate::error::ExampleServerError> {
    let path = config_path_from_args([
        OsString::from("example-server"),
        OsString::from("--config"),
        OsString::from("servers/example/config/example-server.jsonc"),
    ])?;

    assert_eq!(
        path.as_path(),
        Path::new("servers/example/config/example-server.jsonc")
    );
    Ok(())
}

#[test]
fn missing_config_flag_is_rejected() {
    let result = config_path_from_args([OsString::from("example-server")]);

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(ExampleServerErrorReason::ConfigArgumentMissing)
    );
}

#[test]
fn additional_arguments_are_rejected() {
    let result = config_path_from_args([
        OsString::from("example-server"),
        OsString::from("--config"),
        OsString::from("servers/example/config/example-server.jsonc"),
        OsString::from("unexpected"),
    ]);

    assert_eq!(
        result.map_err(|error| error.reason()),
        Err(ExampleServerErrorReason::CommandLineInvalid)
    );
}
