// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::super::{AppHttpMountPath, AppName};
use crate::startup::StartupError;

#[test]
fn app_name_validation_is_typed() {
    assert_eq!(
        AppName::new("api").map(|name| name.as_str().to_owned()),
        Ok("api".to_owned())
    );
    assert_eq!(AppName::new(""), Err(StartupError::EmptyAppName));
    assert_eq!(
        AppName::new("-api"),
        Err(StartupError::InvalidAppNameBoundary)
    );
    assert_eq!(
        AppName::new("ReallyMeApi"),
        Err(StartupError::InvalidAppName)
    );
}

#[test]
fn app_http_mount_path_validation_is_typed() {
    assert_eq!(
        AppHttpMountPath::new("/").map(|path| path.as_str().to_owned()),
        Ok("/".to_owned())
    );
    assert_eq!(
        AppHttpMountPath::new("/internal-admin").map(|path| path.as_str().to_owned()),
        Ok("/internal-admin".to_owned())
    );
    assert_eq!(
        AppHttpMountPath::new("api"),
        Err(StartupError::InvalidAppHttpMountPath)
    );
    assert_eq!(
        AppHttpMountPath::new("/api/"),
        Err(StartupError::InvalidAppHttpMountPath)
    );
    assert_eq!(
        AppHttpMountPath::new("/api?debug=true"),
        Err(StartupError::InvalidAppHttpMountPath)
    );
}
