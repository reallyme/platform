#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import { appendFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const FULL_SHA_PATTERN = /^[0-9a-f]{40}$/u;
const VERSION_PATTERN = /^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$/u;
const MAX_COMMAND_OUTPUT_BYTES = 1_048_576;
const PUBLISHABLE_PACKAGES = Object.freeze([
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
]);

export class ReleaseSourceError extends Error {
  constructor(code) {
    super(code);
    this.name = "ReleaseSourceError";
    this.code = code;
  }
}

const fail = (code) => {
  throw new ReleaseSourceError(code);
};

const run = (command, arguments_, { capture = true } = {}) => {
  const result = spawnSync(command, arguments_, {
    encoding: "utf8",
    maxBuffer: MAX_COMMAND_OUTPUT_BYTES,
    stdio: capture ? ["ignore", "pipe", "ignore"] : "inherit",
  });
  if (result.error !== undefined || result.status !== 0) {
    fail("source-command-failed");
  }
  return capture && typeof result.stdout === "string" ? result.stdout.trim() : "";
};

const readPublishablePackages = () => {
  let metadata;
  try {
    metadata = JSON.parse(
      run("cargo", ["metadata", "--locked", "--format-version", "1", "--no-deps"]),
    );
  } catch (error) {
    if (error instanceof ReleaseSourceError) {
      throw error;
    }
    fail("invalid-cargo-metadata");
  }
  if (metadata === null || typeof metadata !== "object" || !Array.isArray(metadata.packages)) {
    fail("invalid-cargo-metadata");
  }
  return metadata.packages
    .filter((pkg) => Array.isArray(pkg.publish) && pkg.publish.length > 0)
    .map((pkg) => ({ name: pkg.name, version: pkg.version }));
};

export const resolveReleaseVersion = ({ derivesVersion, packages, requestedVersion }) => {
  if (!Array.isArray(packages)) {
    fail("invalid-publishable-package-set");
  }
  const actualNames = packages.map((pkg) => pkg.name).sort();
  if (
    actualNames.length !== PUBLISHABLE_PACKAGES.length ||
    actualNames.some((name, index) => name !== PUBLISHABLE_PACKAGES[index])
  ) {
    fail("invalid-publishable-package-set");
  }
  const versions = packages.map((pkg) => pkg.version);
  const derivedVersion = versions[0];
  if (
    typeof derivedVersion !== "string" ||
    !VERSION_PATTERN.test(derivedVersion) ||
    versions.some((version) => version !== derivedVersion)
  ) {
    fail("manifest-version-mismatch");
  }
  if (
    !derivesVersion &&
    (typeof requestedVersion !== "string" || !VERSION_PATTERN.test(requestedVersion))
  ) {
    fail("invalid-release-version");
  }
  if (!derivesVersion && requestedVersion !== derivedVersion) {
    fail("manifest-version-mismatch");
  }
  return derivesVersion ? derivedVersion : requestedVersion;
};

export const verifyReleaseSource = ({ env = process.env } = {}) => {
  const releaseSha = env.RELEASE_SHA;
  if (typeof releaseSha !== "string" || !FULL_SHA_PATTERN.test(releaseSha)) {
    fail("invalid-release-sha");
  }
  if (env.GITHUB_SHA !== undefined && env.GITHUB_SHA !== releaseSha) {
    fail("workflow-head-mismatch");
  }
  if (run("git", ["rev-parse", "HEAD"]) !== releaseSha) {
    fail("checkout-mismatch");
  }
  run(
    "git",
    ["fetch", "--force", "--no-tags", "origin", "main:refs/remotes/origin/main"],
    { capture: false },
  );
  if (run("git", ["rev-parse", "refs/remotes/origin/main"]) !== releaseSha) {
    fail("origin-main-mismatch");
  }
  const releaseVersion = resolveReleaseVersion({
    derivesVersion: env.RELEASE_SOURCE_DERIVE_VERSION === "1",
    packages: readPublishablePackages(),
    requestedVersion: env.RELEASE_VERSION,
  });
  return { releaseSha, releaseVersion };
};

const isMain = process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  try {
    const identity = verifyReleaseSource();
    if (process.env.RELEASE_SOURCE_WRITE_GITHUB_OUTPUT === "1") {
      const outputPath = process.env.GITHUB_OUTPUT;
      if (typeof outputPath !== "string" || outputPath.length === 0) {
        fail("missing-github-output");
      }
      appendFileSync(
        outputPath,
        `release_sha=${identity.releaseSha}\nrelease_version=${identity.releaseVersion}\n`,
        { encoding: "utf8" },
      );
    }
    console.log("release source matches the workflow head, current main, and crate metadata");
  } catch (error) {
    const code = error instanceof ReleaseSourceError ? error.code : "unexpected-failure";
    console.error(`release source verification failed: ${code}`);
    process.exit(1);
  }
}
