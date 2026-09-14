#!/usr/bin/env node
// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { CratePayloadError, compareCratePayloads } from "./compare_crate_payloads.mjs";

const PACKAGE_NAME = "reallyme-app-kit";
const PACKAGE_VERSION = "0.1.0";
const PACKAGE_ROOT = `${PACKAGE_NAME}-${PACKAGE_VERSION}`;

function writePackage(directory, { vcsSha, workspaceChecksum, source = "pub fn ready() {}\n" }) {
  const packageDirectory = path.join(directory, PACKAGE_ROOT);
  fs.mkdirSync(path.join(packageDirectory, "src"), { recursive: true });
  fs.writeFileSync(
    path.join(packageDirectory, ".cargo_vcs_info.json"),
    `${JSON.stringify({ git: { sha1: vcsSha }, path_in_vcs: "kits/app" }, null, 2)}\n`,
  );
  fs.writeFileSync(
    path.join(packageDirectory, "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "${PACKAGE_NAME}"\n` +
      `version = "${PACKAGE_VERSION}"\nchecksum = "${workspaceChecksum}"\n\n` +
      "[[package]]\nname = \"serde\"\nversion = \"1.0.0\"\n" +
      `checksum = "${"c".repeat(64)}"\n`,
  );
  fs.writeFileSync(path.join(packageDirectory, "Cargo.toml"), "[package]\nname = \"fixture\"\n");
  fs.writeFileSync(path.join(packageDirectory, "src/lib.rs"), source);
}

function createArchive(sourceDirectory, archive) {
  const result = spawnSync("tar", ["-czf", archive, "-C", sourceDirectory, PACKAGE_ROOT], {
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
}

function withArchives(callback) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "crate-payload-test-"));
  try {
    const localSource = path.join(directory, "local-source");
    const publishedSource = path.join(directory, "published-source");
    fs.mkdirSync(localSource);
    fs.mkdirSync(publishedSource);
    writePackage(localSource, {
      vcsSha: "a".repeat(40),
      workspaceChecksum: "1".repeat(64),
    });
    writePackage(publishedSource, {
      vcsSha: "b".repeat(40),
      workspaceChecksum: "2".repeat(64),
    });
    const localArchive = path.join(directory, "local.crate");
    const publishedArchive = path.join(directory, "published.crate");
    createArchive(localSource, localArchive);
    createArchive(publishedSource, publishedArchive);
    callback({ directory, localArchive, publishedArchive, publishedSource });
  } finally {
    fs.rmSync(directory, { force: true, recursive: true });
  }
}

function compare(fixture) {
  compareCratePayloads({
    localArchive: fixture.localArchive,
    publishedArchive: fixture.publishedArchive,
    packageName: PACKAGE_NAME,
    packageVersion: PACKAGE_VERSION,
    workspacePackageNames: [PACKAGE_NAME],
    temporaryDirectory: fixture.directory,
  });
}

test("accepts identical source with different Cargo provenance and workspace checksums", () => {
  withArchives((fixture) => assert.doesNotThrow(() => compare(fixture)));
});

test("rejects a source difference", () => {
  withArchives((fixture) => {
    fs.writeFileSync(
      path.join(fixture.publishedSource, PACKAGE_ROOT, "src/lib.rs"),
      "pub fn changed() {}\n",
    );
    createArchive(fixture.publishedSource, fixture.publishedArchive);
    assert.throws(
      () => compare(fixture),
      (error) => error instanceof CratePayloadError && error.code === "archive-payload-mismatch",
    );
  });
});

test("rejects a dependency lockfile difference outside same-release workspace checksums", () => {
  withArchives((fixture) => {
    const lockPath = path.join(fixture.publishedSource, PACKAGE_ROOT, "Cargo.lock");
    const lock = fs.readFileSync(lockPath, "utf8").replace('version = "1.0.0"', 'version = "1.0.1"');
    fs.writeFileSync(lockPath, lock);
    createArchive(fixture.publishedSource, fixture.publishedArchive);
    assert.throws(
      () => compare(fixture),
      (error) => error instanceof CratePayloadError && error.code === "archive-payload-mismatch",
    );
  });
});
