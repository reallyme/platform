#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

status=0

fail_matches() {
  local description="$1"
  local pattern="$2"
  shift 2

  local output
  if output="$(rg -n "${pattern}" "$@" 2>/dev/null)"; then
    printf '%s\n' "${output}" >&2
    printf '%s\n' "${description}" >&2
    status=1
  fi
}

if ! rg -n '^reallyme-crypto = \{ version = "=0\.4\.2", default-features = false, features = \["native", "dispatch", "ed25519", "hmac", "jwk", "p256", "rsa", "sha2"\] \}' Cargo.toml >/dev/null; then
  printf 'workspace reallyme-crypto dependency must stay pinned to crates.io version 0.4.2 with the audited platform feature set\n' >&2
  status=1
fi

if rg -q '^reallyme-codec\s*=' Cargo.toml &&
  ! rg -q '^reallyme-codec = \{ version = "=0\.4\.3", default-features = false \}' Cargo.toml; then
  printf 'workspace reallyme-codec dependency must use the audited crates.io version 0.4.3\n' >&2
  status=1
fi

fail_matches \
  'use the crates.io reallyme-crypto workspace dependency for primitive cryptographic operations' \
  'reallyme-crypto-(core|ed25519|hmac|p256|sha2-256)' \
  --glob Cargo.toml

fail_matches \
  'do not use sibling checkout path dependencies for reallyme-codec; use crates.io reallyme-codec via workspace dependency' \
  'reallyme-codec\s*=.*path\s*=' \
  --glob Cargo.toml

# ConnectRPC's imported wire codecs retain upstream base64 behavior. Its
# checksum and compatibility patch are verified separately from authored code.
fail_matches \
  'base64/base64url call sites must use reallyme-codec, not the base64 crate directly' \
  '(^|[^[:alnum:]_:])base64::|^[[:space:]]*use base64::' \
  --glob '*.rs' \
  --glob '!conformance/contracts/vendor/connectrpc/src/generated/**'

fail_matches \
  'SHA-2 call sites must use reallyme-crypto::sha2, not the sha2 crate directly' \
  '^[[:space:]]*use sha2::' \
  --glob '*.rs'

fail_matches \
  'ReallyMe cryptographic operations must use the reallyme-crypto umbrella crate, not split primitive crates directly' \
  '\b(crypto_core|crypto_hmac|crypto_ed25519|crypto_p256|crypto_sha2_256)\b' \
  --glob '*.rs'

exit "${status}"
