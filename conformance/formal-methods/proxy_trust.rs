// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

#[path = "../../kits/server/src/http/layers/security/proxy_trust.rs"]
mod trust;

#[kani::proof]
fn proxy_metadata_reader_requires_peer_and_field_authorization() {
    let trusted_peer: bool = kani::any();
    let field_enabled: bool = kani::any();
    let forwarded: Result<Option<u64>, u8> = kani::any();
    let mut read = false;
    let result = trust::read_trusted_metadata(trusted_peer, field_enabled, || {
        read = true;
        forwarded
    });
    if !trusted_peer || !field_enabled {
        assert!(!read);
        assert!(result.is_none());
    } else {
        assert!(read);
        assert_eq!(result, Some(forwarded));
    }
}
