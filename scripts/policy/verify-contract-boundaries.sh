#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe LLC
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"

status=0
temporary_dir="$(mktemp -d)"
readonly temporary_dir
trap 'rm -rf "${temporary_dir}"' EXIT

while IFS= read -r generated_dir; do
  case "${generated_dir}" in
    apps/example/contract/src/generated) ;;
    *)
      printf 'generated Rust directory is outside an approved contract boundary: %s\n' "${generated_dir}" >&2
      status=1
      ;;
  esac
done < <(find apps kits -path '*/src/generated' -type d | sort)

while IFS= read -r contract_manifest; do
  contract_dir="${contract_manifest%/Cargo.toml}"
  if [[ ! -d "${contract_dir}/proto" ]]; then
    printf 'contract crate is missing proto directory: %s\n' "${contract_dir}" >&2
    status=1
  fi
  if [[ ! -f "${contract_dir}/src/generated.rs" ]]; then
    printf 'contract crate is missing generated boundary module: %s\n' "${contract_dir}" >&2
    status=1
  fi
done < <(find apps -path '*/contract/Cargo.toml' -type f | sort)

generated_path_imports_file="${temporary_dir}/generated-path-imports.txt"
if rg -n '#\[path = "generated/' apps/example/src \
  >"${generated_path_imports_file}"; then
  cat "${generated_path_imports_file}" >&2
  printf 'app implementation crates must import generated code through contract crates only\n' >&2
  status=1
fi

exit "${status}"
