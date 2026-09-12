#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./publish_crates_in_order.mjs", import.meta.url));

function runFixture({
  mode = "publish",
  scenario = "success",
  requirement = "^0.1.0",
  alreadyPublishedNames = [],
  dirtyWorktree = false,
} = {}) {
  const directory = mkdtempSync(join(tmpdir(), "platform-publish-test-"));
  try {
    mkdirSync(join(directory, "apps/example/contract/src/generated"), { recursive: true });
    mkdirSync(join(directory, "components/hephaestus/contract/src/generated"), {
      recursive: true,
    });
    const callsPath = join(directory, "calls.json");
    const preload = join(directory, "mock.mjs");
    writeFileSync(
      preload,
      `
import childProcess from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { mkdirSync, writeFileSync } from "node:fs";
const calls = [];
const scenario = ${JSON.stringify(scenario)};
const alreadyPublishedNames = new Set(${JSON.stringify(alreadyPublishedNames)});
const dirtyWorktree = ${JSON.stringify(dirtyWorktree)};
let attempts = 0;
const publicMetadata = (name, dependencies = []) => ({
  name,
  version: "0.1.0",
  publish: ["crates-io"],
  description: name,
  repository: "https://github.com/reallyme/platform",
  license: "MIT OR Apache-2.0",
  dependencies,
});
const localDependency = (name, req = "^0.1.0") => ({
  name,
  source: null,
  path: name,
  kind: null,
  req,
});
Atomics.wait = (_array, _index, _value, delay) => {
  calls.push(["wait", delay]);
  writeFileSync(${JSON.stringify(callsPath)}, JSON.stringify(calls));
  return "timed-out";
};
childProcess.spawnSync = (command, args) => {
  calls.push([command, ...args]);
  writeFileSync(${JSON.stringify(callsPath)}, JSON.stringify(calls));
  const ok = { status: 0, stdout: "", stderr: "" };
  if (command === "git" && args[0] === "status") {
    return { ...ok, stdout: dirtyWorktree ? " M Cargo.toml\\n" : "" };
  }
  if (command === "curl") {
    const outputPath = args[args.indexOf("--output") + 1];
    writeFileSync(outputPath, scenario === "published-mismatch" ? "different" : "archive");
    return ok;
  }
  if (command !== "cargo") return { ...ok, status: 99 };
  if (args[0] === "metadata") return { ...ok, stdout: JSON.stringify({
    target_directory: ${JSON.stringify(directory)},
    packages: [
      publicMetadata("reallyme-app-kit"),
      publicMetadata("reallyme-server-kit", [
        localDependency("reallyme-app-kit", ${JSON.stringify(requirement)}),
      ]),
      publicMetadata("reallyme-hephaestus-domain"),
      publicMetadata("reallyme-hephaestus-contract", [
        localDependency("reallyme-app-kit"),
        localDependency("reallyme-hephaestus-domain"),
      ]),
      publicMetadata("hephaestus-agent", [
        localDependency("reallyme-hephaestus-domain"),
        localDependency("reallyme-hephaestus-contract"),
      ]),
      publicMetadata("reallyme-foundationdb-kit"),
      publicMetadata("reallyme-postgres-kit"),
      publicMetadata("reallyme-nats-kit"),
      publicMetadata("reallyme-typesense-kit"),
      publicMetadata("reallyme-valkey-kit"),
      publicMetadata("reallyme-s3-kit"),
      publicMetadata("reallyme-platform", [
        localDependency("reallyme-app-kit"),
        localDependency("reallyme-server-kit"),
        localDependency("reallyme-hephaestus-domain"),
        localDependency("reallyme-hephaestus-contract"),
        localDependency("reallyme-foundationdb-kit"),
        localDependency("reallyme-postgres-kit"),
        localDependency("reallyme-nats-kit"),
        localDependency("reallyme-typesense-kit"),
        localDependency("reallyme-valkey-kit"),
        localDependency("reallyme-s3-kit"),
      ]),
    ],
  }) };
  if (args[0] === "package") {
    const packageName = args[args.indexOf("-p") + 1];
    if (packageName !== undefined) {
      const packageDirectory = ${JSON.stringify(directory)} + "/package";
      mkdirSync(packageDirectory, { recursive: true });
      writeFileSync(packageDirectory + "/" + packageName + "-0.1.0.crate", "archive");
    }
    return ok;
  }
  if (args[0] !== "publish") return { ...ok, status: 99 };
  const packageName = args[args.indexOf("-p") + 1];
  if (alreadyPublishedNames.has(packageName)) {
    return { ...ok, status: 101, stderr: "crate version already uploaded" };
  }
  attempts += 1;
  if (scenario === "exhausted" || (scenario === "retry" && attempts === 1)) {
    return { ...ok, status: 101, stderr: "too many requests" };
  }
  if (scenario === "failure") {
    return { ...ok, status: 101, stderr: "package verification failed" };
  }
  return ok;
};
syncBuiltinESMExports();
`,
      { encoding: "utf8", mode: 0o600 },
    );
    const result = spawnSync(
      process.execPath,
      ["--import", pathToFileURL(preload).href, script, mode],
      {
        cwd: directory,
        encoding: "utf8",
        timeout: 10_000,
        env: { ...process.env, RELEASE_VERSION: "0.1.0" },
      },
    );
    assert.equal(result.error, undefined);
    return { ...result, calls: JSON.parse(readFileSync(callsPath, "utf8")) };
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

test("successful publication respects every local dependency edge", () => {
  const result = runFixture();
  assert.equal(result.status, 0, result.stderr);
  const published = result.calls
    .filter((call) => call[1] === "publish")
    .map((call) => call[3]);
  const indexOf = (name) => published.indexOf(name);
  assert.ok(indexOf("reallyme-app-kit") < indexOf("reallyme-server-kit"));
  assert.ok(indexOf("reallyme-app-kit") < indexOf("reallyme-hephaestus-contract"));
  assert.ok(indexOf("reallyme-hephaestus-domain") < indexOf("reallyme-hephaestus-contract"));
  assert.ok(indexOf("reallyme-hephaestus-contract") < indexOf("hephaestus-agent"));
  assert.ok(indexOf("reallyme-app-kit") < indexOf("reallyme-platform"));
  assert.ok(indexOf("reallyme-server-kit") < indexOf("reallyme-platform"));
  assert.ok(indexOf("reallyme-s3-kit") < indexOf("reallyme-platform"));
  assert.equal(published.length, 12);
});

test("rate-limit exhaustion stops before dependent publication", () => {
  const result = runFixture({ scenario: "exhausted" });
  assert.equal(result.status, 101);
  const publications = result.calls.filter((call) => call[1] === "publish");
  assert.equal(publications.length, 12);
  assert.ok(publications.every((call) => call[3] === "reallyme-app-kit"));
  assert.equal(result.calls.filter((call) => call[0] === "wait").length, 11);
});

test("transient rate limits retry the current crate", () => {
  const result = runFixture({ scenario: "retry" });
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.calls.filter((call) => call[0] === "wait"), [["wait", 60_000]]);
});

test("existing uploads are skipped while later crates continue publishing", () => {
  const result = runFixture({
    alreadyPublishedNames: [
      "reallyme-app-kit",
      "reallyme-server-kit",
      "reallyme-hephaestus-domain",
    ],
  });
  assert.equal(result.status, 0, result.stderr);
  const published = result.calls
    .filter((call) => call[1] === "publish")
    .map((call) => call[3]);
  assert.ok(published.includes("reallyme-hephaestus-contract"));
  assert.ok(published.includes("hephaestus-agent"));
  assert.ok(published.includes("reallyme-platform"));
  assert.equal(published.length, 12);
  assert.equal(result.calls.filter((call) => call[0] === "curl").length, 3);
});

test("an existing upload from different source bytes fails closed", () => {
  const result = runFixture({
    scenario: "published-mismatch",
    alreadyPublishedNames: ["reallyme-app-kit"],
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /different source bytes/u);
  assert.equal(result.calls.filter((call) => call[1] === "publish").length, 1);
});

test("publication refuses a dirty worktree before reading Cargo metadata", () => {
  const result = runFixture({ dirtyWorktree: true });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /completely clean Git worktree/u);
  assert.equal(result.calls.filter((call) => call[0] === "cargo").length, 0);
});

test("non-retryable publication errors fail immediately", () => {
  const result = runFixture({ scenario: "failure" });
  assert.equal(result.status, 101);
  assert.equal(result.calls.filter((call) => call[1] === "publish").length, 1);
  assert.equal(result.calls.filter((call) => call[0] === "wait").length, 0);
});

test("incompatible workspace requirements fail before packaging", () => {
  const result = runFixture({ mode: "order", requirement: "^0.2.0" });
  assert.equal(result.status, 1);
  assert.ok(result.calls.every((call) => call[1] === "metadata"));
});
