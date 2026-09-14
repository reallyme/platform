// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppVersion;
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn validates_app_version() {
    let version = AppVersion::new("0.1.0").expect("valid app version");

    assert_eq!(version.as_str(), "0.1.0");
}

#[test]
fn validates_app_version_with_prerelease() {
    let version = AppVersion::new("1.2.3-rc.1").expect("valid semver prerelease");

    assert_eq!(version.as_str(), "1.2.3-rc.1");
}

#[test]
fn rejects_version_with_whitespace() {
    assert_eq!(
        AppVersion::new("0.1.0 dev"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}

#[test]
fn rejects_version_that_is_not_semver_shape() {
    assert_eq!(
        AppVersion::new("!@#$%"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
    assert_eq!(
        AppVersion::new("helloworld"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
    assert_eq!(
        AppVersion::new("1.2"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
    assert_eq!(
        AppVersion::new("1.02.3"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
    assert_eq!(
        AppVersion::new("1.2.3+abc"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
    assert_eq!(
        AppVersion::new("💩💩💩"),
        Err(AppKitError::new(
            AppKitField::AppVersion,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}
