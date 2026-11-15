#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

if [[ "$#" -ne 2 || ( "$1" != --check && "$1" != --write ) ]]; then
  printf 'usage: %s {--check|--write} <connectrpc-0.9.1.crate>\n' "$0" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
vendor="${repo_root}/conformance/contracts/vendor/connectrpc"
readonly expected_sha=457cfa540c62e1b0b05db676cc9a9129162ad94d3a3eec6f2d19706c53acae7d

if command -v sha256sum >/dev/null 2>&1; then
  digest="$(sha256sum < "$2")"
else
  digest="$(shasum -a 256 < "$2")"
fi
if [[ "${digest%% *}" != "${expected_sha}" ]]; then
  printf 'ConnectRPC registry archive checksum does not match the reviewed source\n' >&2
  exit 1
fi

scratch="$(mktemp -d)"
trap 'rm -rf -- "${scratch}"' EXIT
tar -xzf "$2" -C "${scratch}" connectrpc-0.9.1/src
source_dir="${scratch}/connectrpc-0.9.1/src"
(
  cd "${source_dir}"
  git apply --no-index "${vendor}/buffa-compatibility.patch"
)

generated_dir="${vendor}/src/generated"
case "$1" in
  --check) diff -r "${source_dir}" "${generated_dir}" ;;
  --write)
    # Replace only the derived source tree, after verifying the archive and
    # applying the reviewable compatibility patch in temporary storage.
    rm -rf -- "${generated_dir}"
    mkdir -p "${generated_dir}"
    cp -R "${source_dir}/." "${generated_dir}/"
    ;;
esac
