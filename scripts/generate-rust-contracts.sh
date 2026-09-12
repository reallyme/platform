#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

readonly contract_dirs=(
  "apps/example/contract"
  "components/hephaestus/contract"
)

for contract_dir in "${contract_dirs[@]}"; do
  if [[ ! -f "${repo_root}/${contract_dir}/buf.gen.yaml" ]]; then
    printf 'missing buf.gen.yaml for %s\n' "${contract_dir}" >&2
    exit 1
  fi

  generated_dir="${repo_root}/${contract_dir}/src/generated"
  # Contract output is reviewed source distributed with its contract crate.
  # Remove the exact output tree first so schema deletions cannot leave stale
  # modules behind; CI then verifies it matches the committed source.
  rm -rf "${generated_dir}"

  printf 'Generating Rust contract code for %s\n' "${contract_dir}"
  if [[ -f "${repo_root}/${contract_dir}/buf.gen.rust.yaml" ]]; then
    (cd "${repo_root}/${contract_dir}" && buf generate --template buf.gen.rust.yaml)
  else
    (cd "${repo_root}/${contract_dir}" && buf generate)
  fi

  if [[ ! -d "${generated_dir}" ]]; then
    printf 'missing generated directory for %s\n' "${contract_dir}" >&2
    exit 1
  fi

  contract_parent="${repo_root}/${contract_dir%/contract}"
  license_stamper="${contract_parent}/scripts/stamp-generated-license-headers.py"
  if [[ -f "${license_stamper}" ]]; then
    python3 "${license_stamper}"
  fi

  # The generators emit valid Rust, but rustfmt still owns repository style.
  while IFS= read -r -d '' rust_file; do
    rustfmt --edition 2024 "${rust_file}"
  done < <(find "${generated_dir}" -name '*.rs' -print0)
done
