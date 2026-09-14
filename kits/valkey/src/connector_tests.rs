// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_TLS_CA_PEM_BYTES, decode_deleted_count, read_custom_root};
use crate::{ValkeyCommandErrorReason, ValkeyError, ValkeySetupErrorReason};

#[test]
fn delete_count_rejects_protocol_violation() {
    assert_eq!(decode_deleted_count(0), Ok(false));
    assert_eq!(decode_deleted_count(1), Ok(true));
    assert_eq!(
        decode_deleted_count(2),
        Err(ValkeyError::Command {
            reason: ValkeyCommandErrorReason::InvalidResponse,
        })
    );
}

#[test]
fn custom_root_reader_rejects_missing_empty_and_oversized_files() {
    let directory = tempfile::tempdir().expect("temporary directory should be available");
    let missing = directory.path().join("missing.pem");
    assert_eq!(
        read_custom_root(&missing).err(),
        Some(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::TlsTrustUnavailable,
        })
    );

    let empty = directory.path().join("empty.pem");
    std::fs::write(&empty, []).expect("empty fixture should be written");
    assert_eq!(
        read_custom_root(&empty).err(),
        Some(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::TlsTrustInvalid,
        })
    );

    let oversized = directory.path().join("oversized.pem");
    let file = std::fs::File::create(&oversized).expect("oversized fixture should be created");
    file.set_len(MAX_TLS_CA_PEM_BYTES + 1)
        .expect("oversized fixture should be sized");
    assert_eq!(
        read_custom_root(&oversized).err(),
        Some(ValkeyError::Setup {
            reason: ValkeySetupErrorReason::TlsTrustTooLarge,
        })
    );
}
