#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 ReallyMe Technologies Limited
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repo_root}"
scratch="$(mktemp -d)"
trap 'rm -rf -- "${scratch}"' EXIT

contract_dirs=(apps/example/contract apps/platform/contract)
generated_dirs=(apps/example/contract/src/generated kits/app/src/generated kits/foundationdb/src/generated)
release_tag="$(git describe --tags --match 'v[0-9]*' --abbrev=0)"
release_sha="$(git rev-parse --verify "${release_tag}^{commit}")"
base_sha="${CONTRACT_BASELINE_SHA:-}"
if [[ -z "${base_sha}" || "${base_sha}" == 0000000000000000000000000000000000000000 ]]; then
  base_sha="$(git rev-parse --verify 'HEAD^1^{commit}')"
fi
if [[ ! "${base_sha}" =~ ^[0-9a-f]{40}$ ]] ||
  [[ "$(git rev-parse --verify "${base_sha}^{commit}")" != "${base_sha}" ]]; then
  printf 'contract baseline must be an available commit SHA\n' >&2
  exit 1
fi

baselines=("${release_sha}")
if [[ "${base_sha}" != "${release_sha}" ]]; then
  baselines+=("${base_sha}")
fi

for contract in "${contract_dirs[@]}"; do
  buf lint "${contract}"
  for baseline in "${baselines[@]}"; do
    if [[ -z "$(git ls-tree -r --name-only "${baseline}" -- "${contract}/proto")" ]]; then
      printf '%s introduces schemas after %s\n' "${contract}" "${baseline}"
      continue
    fi
    # Compare against a known tree, even when a PR moves or deletes a file.
    baseline_dir="${scratch}/${baseline}/${contract}"
    mkdir -p "${baseline_dir}"
    git archive "${baseline}" -- "${contract}/buf.yaml" "${contract}/proto" |
      tar -xf - -C "${scratch}/${baseline}"
    buf breaking "${contract}" --against "${baseline_dir}"
  done
done

scripts/generation/generate-rust-contracts.sh
git diff --exit-code -- "${generated_dirs[@]}"
test -z "$(git ls-files --others --exclude-standard -- "${generated_dirs[@]}")"
cargo +1.100.0 check --locked \
  -p reallyme-app-kit -p reallyme-example-contract -p reallyme-foundationdb-kit \
  --features reallyme-app-kit/http-contract,reallyme-example-contract/connect,reallyme-foundationdb-kit/tenant-admin
