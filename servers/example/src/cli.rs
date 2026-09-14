// SPDX-FileCopyrightText: 2026 ReallyMe LLC
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
#[path = "cli_tests.rs"]
mod tests;
