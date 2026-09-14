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

readonly production_maximum_lines=500
readonly test_maximum_lines=800
readonly absolute_maximum_lines=1200

while IFS= read -r generated_path; do
  # During a crate extraction, the index retains deleted paths until the
  # migration is staged. Only source files present in the working tree are
  # meaningful inputs to this audit.
  if [[ ! -f "${generated_path}" ]]; then
    continue
  fi
  case "${generated_path}" in
    apps/example/contract/src/generated/*) ;;
    *)
      printf 'generated Rust source is outside an approved contract: %s\n' \
        "${generated_path}" >&2
      status=1
      ;;
  esac
done < <(git ls-files | rg '/src/generated/|/generated/(buffa|connect|grpc)/')

generated_directory="apps/example/contract/src/generated"
if ! git ls-files -- "${generated_directory}" | rg -q .; then
  printf 'contract generated source must be tracked: %s\n' "${generated_directory}" >&2
  status=1
fi

while IFS= read -r rust_file; do
  # `git ls-files` retains paths deleted by an unstaged crate extraction. There
  # is no source file to audit in that state, and attempting to read it makes
  # this working-tree check fail before it can inspect the remaining sources.
  if [[ ! -f "${rust_file}" ]]; then
    continue
  fi
  first_line="$(sed -n '1p' "${rust_file}")"
  second_line="$(sed -n '2p' "${rust_file}")"
  if [[ "${first_line}" != '// SPDX-FileCopyrightText: 2026 ReallyMe LLC' ]] || \
    [[ "${second_line}" != '// SPDX-License-Identifier: MIT OR Apache-2.0' ]]; then
    printf 'missing exact ReallyMe SPDX header: %s\n' "${rust_file}" >&2
    status=1
  fi
done < <(git ls-files '*.rs' | rg -v '/generated/')

ds_store_paths_file="${temporary_dir}/ds-store-tracked.txt"
if git ls-files | rg '(^|/)\.DS_Store$' >"${ds_store_paths_file}"; then
  cat "${ds_store_paths_file}" >&2
  printf '.DS_Store files must not be tracked\n' >&2
  status=1
fi

is_public_manifest() {
  case "$1" in
    ./crates/platform/Cargo.toml | \
      ./kits/app/Cargo.toml | \
      ./kits/foundationdb/Cargo.toml | \
      ./kits/nats/Cargo.toml | \
      ./kits/postgres/Cargo.toml | \
      ./kits/s3/Cargo.toml | \
      ./kits/server/Cargo.toml | \
      ./kits/typesense/Cargo.toml | \
      ./kits/valkey/Cargo.toml)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

while IFS= read -r cargo_file; do
  if [[ "${cargo_file}" == ./Cargo.toml ]]; then
    continue
  elif is_public_manifest "${cargo_file}"; then
    if ! rg -n '^publish = \["crates-io"\]$' "${cargo_file}" >/dev/null; then
      printf 'approved public crate must publish only to crates.io: %s\n' "${cargo_file}" >&2
      status=1
    fi
    for required_metadata in '^description = ' '^repository\.workspace = true$'; do
      if ! rg -n "${required_metadata}" "${cargo_file}" >/dev/null; then
        printf 'public crate is missing release metadata (%s): %s\n' \
          "${required_metadata}" "${cargo_file}" >&2
        status=1
      fi
    done
  elif ! rg -n '^publish = false$' "${cargo_file}" >/dev/null; then
    printf 'unapproved workspace crate must remain private: %s\n' "${cargo_file}" >&2
    status=1
  fi
done < <(find . -name Cargo.toml -not -path './target/*' | sort)

while IFS= read -r member_manifest; do
  if ! rg -q '^\[lints\][[:space:]]*$' "${member_manifest}" ||
    ! sed -n '/^\[lints\][[:space:]]*$/,/^\[/p' "${member_manifest}" |
      rg -q '^workspace = true$'; then
    printf 'workspace member must inherit workspace lints: %s\n' "${member_manifest}" >&2
    status=1
  fi
done < <(find apps crates kits servers workers -name Cargo.toml -type f | sort)

while IFS= read -r rust_path; do
  case "${rust_path}" in
    apps/example/contract/src/generated/*) continue ;;
    */tests/* | */tests.rs | *_tests.rs) maximum_lines="${test_maximum_lines}" ;;
    *) maximum_lines="${production_maximum_lines}" ;;
  esac

  actual_lines="$(wc -l < "${rust_path}")"
  if (( actual_lines > absolute_maximum_lines )); then
    printf 'authored Rust source exceeds absolute audit limit: %s (%s > %s)\n' \
      "${rust_path}" "${actual_lines}" "${absolute_maximum_lines}" >&2
    status=1
    continue
  fi
  if (( actual_lines <= maximum_lines )); then
    continue
  fi
  printf 'authored Rust source exceeds audit limit: %s (%s > %s)\n' \
    "${rust_path}" "${actual_lines}" "${maximum_lines}" >&2
  status=1
done < <(rg --files apps crates kits servers workers -g '*.rs')

while IFS= read -r wildcard_import; do
  printf 'wildcard Rust import is forbidden: %s\n' "${wildcard_import}" >&2
  status=1
done < <(rg -n '^[[:space:]]*(pub[[:space:]]+)?use[[:space:]][^;]*::[*][[:space:]]*;' \
  apps crates kits servers workers --glob '*.rs' \
  --glob '!apps/example/contract/src/generated/**' || true)

if ! scripts/policy/verify-crypto-policy.sh; then
  status=1
fi

exit "${status}"
