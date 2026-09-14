// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{AppErrorCategory, AppErrorCode, AppErrorContract, AppErrorRetryDisposition};
use crate::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn app_error_contract_is_typed_and_low_cardinality() {
    let code = AppErrorCode::new("invalid_request").expect("valid app error code fixture");
    let contract = AppErrorContract::new(
        code,
        AppErrorCategory::InvalidRequest,
        AppErrorRetryDisposition::NotRetryable,
    );

    assert_eq!(contract.code().as_str(), "invalid_request");
    assert_eq!(contract.category(), AppErrorCategory::InvalidRequest);
}

#[test]
fn rejects_error_codes_with_spaces() {
    assert_eq!(
        AppErrorCode::new("invalid request"),
        Err(AppKitError::new(
            AppKitField::ErrorCode,
            AppKitErrorReason::InvalidCharacter,
        ))
    );
}
