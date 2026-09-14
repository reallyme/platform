// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::AppContractName;
use crate::{AppKitError, AppKitErrorReason, AppKitField};

#[test]
fn accepts_stable_contract_names() {
    let name = AppContractName::new("reallyme-api-contract").expect("valid contract name");

    assert_eq!(name.as_str(), "reallyme-api-contract");
}

#[test]
fn rejects_dynamic_or_invalid_contract_names() {
    assert_eq!(
        AppContractName::new("reallyme/api"),
        Err(AppKitError::new(
            AppKitField::ContractName,
            AppKitErrorReason::InvalidCharacter,
        )),
    );
    assert_eq!(
        AppContractName::new("123-contract"),
        Err(AppKitError::new(
            AppKitField::ContractName,
            AppKitErrorReason::InvalidCharacter,
        )),
    );
}
