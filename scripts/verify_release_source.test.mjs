#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import {
  ReleaseSourceError,
  resolveReleaseVersion,
  verifyReleaseSource,
} from "./verify_release_source.mjs";

const names = [
  "hephaestus-agent",
  "reallyme-app-kit",
  "reallyme-foundationdb-kit",
  "reallyme-hephaestus-contract",
  "reallyme-hephaestus-domain",
  "reallyme-nats-kit",
  "reallyme-postgres-kit",
  "reallyme-s3-kit",
  "reallyme-server-kit",
  "reallyme-typesense-kit",
  "reallyme-valkey-kit",
];
const packages = (version = "0.1.0") => names.map((name) => ({ name, version }));

function releaseSourceRunner({ dirty = false, releaseSha }) {
  return (command, arguments_) => {
    if (command === "git" && arguments_[0] === "rev-parse") {
      return releaseSha;
    }
    if (command === "git" && arguments_[0] === "status") {
      return dirty ? " M Cargo.toml" : "";
    }
    if (command === "git" && arguments_[0] === "fetch") {
      return "";
    }
    if (command === "cargo" && arguments_[0] === "metadata") {
      return JSON.stringify({
        packages: packages().map((pkg) => ({ ...pkg, publish: ["crates-io"] })),
      });
    }
    throw new Error("unexpected fixture command");
  };
}

test("release version is derived only when every public crate agrees", () => {
  assert.equal(
    resolveReleaseVersion({
      derivesVersion: true,
      packages: packages(),
      requestedVersion: undefined,
    }),
    "0.1.0",
  );
  const mismatched = packages();
  mismatched[0] = { ...mismatched[0], version: "0.1.1" };
  assert.throws(
    () =>
      resolveReleaseVersion({
        derivesVersion: true,
        packages: mismatched,
        requestedVersion: undefined,
      }),
    ReleaseSourceError,
  );
});

test("explicit preflight version remains bound to public crate metadata", () => {
  assert.equal(
    resolveReleaseVersion({
      derivesVersion: false,
      packages: packages(),
      requestedVersion: "0.1.0",
    }),
    "0.1.0",
  );
  for (const requestedVersion of [undefined, "v0.1.0", "0.1.1"]) {
    assert.throws(
      () =>
        resolveReleaseVersion({
          derivesVersion: false,
          packages: packages(),
          requestedVersion,
        }),
      ReleaseSourceError,
    );
  }
});

test("an unexpected public or private package fails closed", () => {
  assert.throws(
    () =>
      resolveReleaseVersion({
        derivesVersion: true,
        packages: [...packages(), { name: "example-server", version: "0.1.0" }],
        requestedVersion: undefined,
      }),
    ReleaseSourceError,
  );
  assert.throws(
    () =>
      resolveReleaseVersion({
        derivesVersion: true,
        packages: packages().slice(1),
        requestedVersion: undefined,
      }),
    ReleaseSourceError,
  );
});

test("release source verification accepts only a clean exact checkout", () => {
  const releaseSha = "a".repeat(40);
  const env = {
    GITHUB_SHA: releaseSha,
    RELEASE_SHA: releaseSha,
    RELEASE_SOURCE_DERIVE_VERSION: "1",
  };
  assert.deepEqual(
    verifyReleaseSource({
      commandRunner: releaseSourceRunner({ releaseSha }),
      env,
    }),
    { releaseSha, releaseVersion: "0.1.0" },
  );
  assert.throws(
    () =>
      verifyReleaseSource({
        commandRunner: releaseSourceRunner({ dirty: true, releaseSha }),
        env,
      }),
    (error) => error instanceof ReleaseSourceError && error.code === "dirty-release-worktree",
  );
});
