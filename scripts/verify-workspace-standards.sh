#!/usr/bin/env bash
# SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

status=0
temporary_dir="$(mktemp -d)"
readonly temporary_dir
trap 'rm -rf "${temporary_dir}"' EXIT

while IFS= read -r generated_path; do
  case "${generated_path}" in
    apps/example/contract/src/generated/* | components/hephaestus/contract/src/generated/*) ;;
    *)
      printf 'generated Rust source is outside an approved contract: %s\n' \
        "${generated_path}" >&2
      status=1
      ;;
  esac
done < <(git ls-files | rg '/src/generated/|/generated/(buffa|connect|grpc)/')

for generated_directory in \
  apps/example/contract/src/generated \
  components/hephaestus/contract/src/generated; do
  if ! git ls-files -- "${generated_directory}" | rg -q .; then
    printf 'contract generated source must be tracked: %s\n' "${generated_directory}" >&2
    status=1
  fi
done

while IFS= read -r rust_file; do
  # `git ls-files` retains paths deleted by an unstaged crate extraction. There
  # is no source file to audit in that state, and attempting to read it makes
  # this working-tree check fail before it can inspect the remaining sources.
  if [[ ! -f "${rust_file}" ]]; then
    continue
  fi
  first_line="$(sed -n '1p' "${rust_file}")"
  case "${first_line}" in
    '// SPDX-FileCopyrightText:'*) ;;
    *)
      printf 'missing ReallyMe SPDX header: %s\n' "${rust_file}" >&2
      status=1
      ;;
  esac
done < <(git ls-files '*.rs' | rg -v '/generated/')

ds_store_paths_file="${temporary_dir}/ds-store-tracked.txt"
if git ls-files | rg '(^|/)\.DS_Store$' >"${ds_store_paths_file}"; then
  cat "${ds_store_paths_file}" >&2
  printf '.DS_Store files must not be tracked\n' >&2
  status=1
fi

is_public_manifest() {
  case "$1" in
    ./Cargo.toml | \
      ./components/hephaestus/agent/Cargo.toml | \
      ./components/hephaestus/contract/Cargo.toml | \
      ./components/hephaestus/domain/Cargo.toml | \
      ./kits/reallyme-app-kit/Cargo.toml | \
      ./kits/reallyme-foundationdb-kit/Cargo.toml | \
      ./kits/reallyme-nats-kit/Cargo.toml | \
      ./kits/reallyme-postgres-kit/Cargo.toml | \
      ./kits/reallyme-s3-kit/Cargo.toml | \
      ./kits/reallyme-server-kit/Cargo.toml | \
      ./kits/reallyme-typesense-kit/Cargo.toml | \
      ./kits/reallyme-valkey-kit/Cargo.toml)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

while IFS= read -r cargo_file; do
  if is_public_manifest "${cargo_file}"; then
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

if ! scripts/verify-crypto-policy.sh; then
  status=1
fi

exit "${status}"
