// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::{ExampleServerError, ExampleServerErrorReason};

pub(crate) fn config_path_from_args<I>(args: I) -> Result<PathBuf, ExampleServerError>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _binary_name = args
        .next()
        .ok_or_else(|| ExampleServerError::new(ExampleServerErrorReason::ConfigArgumentMissing))?;
    let flag = args
        .next()
        .ok_or_else(|| ExampleServerError::new(ExampleServerErrorReason::ConfigArgumentMissing))?;

    if flag != "--config" {
        return Err(ExampleServerError::new(
            ExampleServerErrorReason::CommandLineInvalid,
        ));
    }

    let path = args
        .next()
        .ok_or_else(|| ExampleServerError::new(ExampleServerErrorReason::ConfigArgumentMissing))?;
    if args.next().is_some() {
        return Err(ExampleServerError::new(
            ExampleServerErrorReason::CommandLineInvalid,
        ));
    }

    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::config_path_from_args;
    use crate::error::ExampleServerErrorReason;

    #[test]
    fn config_flag_selects_server_document() -> Result<(), crate::error::ExampleServerError> {
        let path = config_path_from_args([
            OsString::from("example-server"),
            OsString::from("--config"),
            OsString::from("servers/configs/example-server.jsonc"),
        ])?;

        assert_eq!(
            path.as_path(),
            Path::new("servers/configs/example-server.jsonc")
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
            OsString::from("servers/configs/example-server.jsonc"),
            OsString::from("unexpected"),
        ]);

        assert_eq!(
            result.map_err(|error| error.reason()),
            Err(ExampleServerErrorReason::CommandLineInvalid)
        );
    }
}
