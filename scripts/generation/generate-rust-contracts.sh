#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly example_contract="apps/example/contract"
readonly platform_contract="apps/platform/contract"
readonly http_schema="proto/reallyme/platform/http/v1"
readonly storage_schema="proto/reallyme/platform/storage/v1"

readonly buffa_version=0.1.2
for plugin in protoc-gen-reallyme-buffa protoc-gen-reallyme-buffa-packaging; do
  if ! actual_version="$("${plugin}" --version)" ||
    [[ "${actual_version}" != "${plugin} ${buffa_version}" ]]; then
    printf 'contract generation requires %s %s\n' "${plugin}" "${buffa_version}" >&2
    exit 1
  fi
done

list_schema_inputs() {
  local contract="$1"
  shift
  # Build with the same module and path selection that generation uses.
  # Imported schemas, including Google's well-known types, are not ours to generate.
  (cd "${repo_root}/${contract}" &&
    buf build --as-file-descriptor-set --exclude-imports --disable-symlinks "$@" -o '-#format=json') |
    jq -r --arg prefix "${contract}/proto/" '.file[].name | $prefix + .'
}

scratch="$(mktemp -d)"
trap 'rm -rf -- "${scratch}"' EXIT
git -C "${repo_root}" ls-files --cached --others --exclude-standard -z -- '*.proto' >"${scratch}/paths"
while IFS= read -r -d '' schema; do
  # A deleted tracked file stays in the index until the change is staged.
  if [[ -e "${repo_root}/${schema}" || -L "${repo_root}/${schema}" ]]; then
    printf '%s\n' "${schema}"
  fi
done <"${scratch}/paths" | LC_ALL=C sort -u >"${scratch}/repository"
{
  list_schema_inputs "${example_contract}"
  list_schema_inputs "${platform_contract}" --path "${http_schema}"
  list_schema_inputs "${platform_contract}" --path "${storage_schema}"
} >"${scratch}/inputs"
LC_ALL=C sort -u "${scratch}/inputs" >"${scratch}/generation"
if ! diff -u "${scratch}/repository" "${scratch}/generation"; then
  printf 'every repository protobuf schema must have a Rust generation target\n' >&2
  exit 1
fi

readonly contract_dirs=(
  "${example_contract}"
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
    (cd "${repo_root}/${contract_dir}" && buf generate --disable-symlinks --template buf.gen.rust.yaml)
  else
    (cd "${repo_root}/${contract_dir}" && buf generate --disable-symlinks)
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

generate_kit_bindings() {
  local schema="$1" template="$2" output="$3"
  rm -rf -- "${repo_root:?}/${output:?}"
  (cd "${repo_root}/${platform_contract}" && buf generate --disable-symlinks --path "${schema}" \
    --template "${template}" --output "${repo_root}")
  if [[ ! -f "${repo_root}/${output}/mod.rs" ]]; then
    printf 'missing generated package for %s\n' "${schema}" >&2
    return 1
  fi
  while IFS= read -r -d '' rust_file; do
    rustfmt --edition 2024 "${rust_file}"
  done < <(find "${repo_root}/${output}" -name '*.rs' -print0)
}

generate_kit_bindings "${http_schema}" buf.gen.http.yaml kits/app/src/generated
generate_kit_bindings "${storage_schema}" buf.gen.storage.yaml kits/foundationdb/src/generated
