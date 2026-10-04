// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use connectrpc::{ConnectError, ErrorCode};
use reallyme_app_kit::AppErrorCategory;

const FORBIDDEN_MESSAGE: &str = "Hello is disabled";
const REQUEST_FAILED_MESSAGE: &str = "Request could not be completed";
const INTERNAL_MESSAGE: &str = "Internal server error";

pub(super) fn map_example_app_error_to_connect_error(
    error: crate::app::ExampleAppError,
) -> ConnectError {
    let Ok(contract) = error.contract() else {
        return ConnectError::new(ErrorCode::Internal, INTERNAL_MESSAGE);
    };
    let (code, message) = match contract.category() {
        AppErrorCategory::InvalidRequest => (ErrorCode::InvalidArgument, REQUEST_FAILED_MESSAGE),
        AppErrorCategory::Unauthenticated => (ErrorCode::Unauthenticated, REQUEST_FAILED_MESSAGE),
        AppErrorCategory::PermissionDenied => (ErrorCode::PermissionDenied, FORBIDDEN_MESSAGE),
        AppErrorCategory::NotFound => (ErrorCode::NotFound, REQUEST_FAILED_MESSAGE),
        AppErrorCategory::Conflict => (ErrorCode::AlreadyExists, REQUEST_FAILED_MESSAGE),
        AppErrorCategory::ResourceExhausted => {
            (ErrorCode::ResourceExhausted, REQUEST_FAILED_MESSAGE)
        }
        AppErrorCategory::Unavailable => (ErrorCode::Unavailable, REQUEST_FAILED_MESSAGE),
        AppErrorCategory::DeadlineExceeded => (ErrorCode::DeadlineExceeded, REQUEST_FAILED_MESSAGE),
        AppErrorCategory::Internal => (ErrorCode::Internal, INTERNAL_MESSAGE),
    };
    ConnectError::new(code, message)
}

#[cfg(test)]
mod tests {
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
            map_example_app_error_to_connect_error(ExampleAppError::MetricConfigurationInvalid)
                .code,
            ErrorCode::Internal
        );
        assert_eq!(
            map_example_app_error_to_connect_error(ExampleAppError::DeadlineExceeded).code,
            ErrorCode::DeadlineExceeded
        );
    }
}
