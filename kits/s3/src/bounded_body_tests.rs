// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

use zeroize::Zeroizing;

use super::append_sensitive_chunk;
use crate::S3StorageErrorReason;

#[test]
fn growth_preserves_bytes_without_reserving_the_full_limit() {
    let mut body = Zeroizing::new(Vec::new());
    append_sensitive_chunk(&mut body, b"ab", 1_024).expect("first chunk");
    append_sensitive_chunk(&mut body, b"cdef", 1_024).expect("second chunk");
    assert_eq!(body.as_slice(), b"abcdef");
    assert!(body.capacity() < 1_024);
    assert!(matches!(
        append_sensitive_chunk(&mut body, b"ghi", 8),
        Err(error) if error.reason == S3StorageErrorReason::ObjectTooLarge
    ));
}
