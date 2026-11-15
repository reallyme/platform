// SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
// SPDX-License-Identifier: MIT OR Apache-2.0

/// Reads proxy metadata only when the transport peer and the field policy
/// both authorize it. Laziness keeps even malformed untrusted input local
/// to the ignored headers.
pub(crate) fn read_trusted_metadata<T>(
    trusted_peer: bool,
    field_enabled: bool,
    read: impl FnOnce() -> T,
) -> Option<T> {
    (trusted_peer && field_enabled).then(read)
}
