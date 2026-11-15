// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Authorizes the principal returned by this request's authenticator.
/// Transport extensions are populated only after both policies agree.
pub(super) fn authorize_call<Principal, Error>(
    authenticate: impl FnOnce() -> Result<Principal, Error>,
    authorize: impl FnOnce(&Principal) -> Result<(), Error>,
) -> Result<Principal, Error> {
    let principal = authenticate()?;
    authorize(&principal)?;
    Ok(principal)
}
