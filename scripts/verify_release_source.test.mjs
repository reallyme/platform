#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";

import { ReleaseSourceError, resolveReleaseVersion } from "./verify_release_source.mjs";

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
