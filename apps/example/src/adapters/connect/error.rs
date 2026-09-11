// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use connectrpc::{ConnectError, ErrorCode};

const EXAMPLE_APP_UNAVAILABLE_MESSAGE: &str = "Example app is unavailable";

pub(super) fn map_example_app_error_to_connect_error(
    _error: crate::app::ExampleAppError,
) -> ConnectError {
    ConnectError::new(ErrorCode::PermissionDenied, EXAMPLE_APP_UNAVAILABLE_MESSAGE)
}
