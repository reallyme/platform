// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use connectrpc::ErrorCode;

use super::map_example_app_error_to_connect_error;
use crate::app::ExampleAppError;

#[test]
fn app_error_categories_map_to_distinct_connect_statuses() {
    assert_eq!(
        map_example_app_error_to_connect_error(ExampleAppError::HelloDisabled).code,
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        map_example_app_error_to_connect_error(ExampleAppError::MetricConfigurationInvalid).code,
        ErrorCode::Internal
    );
    assert_eq!(
        map_example_app_error_to_connect_error(ExampleAppError::DeadlineExceeded).code,
        ErrorCode::DeadlineExceeded
    );
}
