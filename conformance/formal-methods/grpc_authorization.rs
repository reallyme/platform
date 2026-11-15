// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/server/src/grpc/authorize_call.rs"]
mod binding;

#[kani::proof]
fn dispatch_requires_both_policies_and_preserves_the_principal() {
    let authentication: Result<u64, u8> = kani::any();
    let authorization: Result<(), u8> = kani::any();
    let mut seen = None;
    let result = binding::authorize_call(
        || authentication,
        |principal| {
            seen = Some(*principal);
            authorization
        },
    );
    match authentication {
        Err(error) => {
            assert!(seen.is_none());
            assert_eq!(result, Err(error));
        }
        Ok(principal) => {
            assert_eq!(seen, Some(principal));
            assert_eq!(result, authorization.map(|()| principal));
        }
    }
}
