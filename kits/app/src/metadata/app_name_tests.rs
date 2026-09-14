// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppName;
use crate::error::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn validates_app_names() {
    let name = AppName::new("reallyme-api").expect("valid app name");

    assert_eq!(name.as_str(), "reallyme-api");
}

#[test]
fn rejects_invalid_app_names() {
    assert_eq!(
        AppName::new("ReallyMe API"),
        Err(AppKitError::new(
            AppKitField::AppName,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}
