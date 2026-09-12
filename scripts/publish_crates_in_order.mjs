#!/usr/bin/env node
// SPDX-FileCopyrightText: Copyright © 2026 ReallyMe LLC. All rights reserved
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

import { CratePayloadError, compareCratePayloads } from "./compare_crate_payloads.mjs";

const MODE_INSPECT = "inspect";
const MODE_ORDER = "order";
const MODE_PUBLISH = "publish";
const MAX_PUBLISH_ATTEMPTS = 12;
const CRATES_IO_DEFAULT_RATE_LIMIT_RETRY_MS = 60_000;
const CRATES_IO_INDEX_RETRY_BASE_MS = 15_000;
const EXPECTED_PUBLISHABLE_PACKAGES = Object.freeze([
  "hephaestus-agent",
  "reallyme-app-kit",
  "reallyme-foundationdb-kit",
  "reallyme-hephaestus-contract",
  "reallyme-hephaestus-domain",
  "reallyme-nats-kit",
  "reallyme-platform",
  "reallyme-postgres-kit",
  "reallyme-s3-kit",
  "reallyme-server-kit",
  "reallyme-typesense-kit",
  "reallyme-valkey-kit",
]);
const REQUIRED_PUBLISH_ORDER_EDGES = Object.freeze([
  ["reallyme-app-kit", "reallyme-hephaestus-contract"],
  ["reallyme-app-kit", "reallyme-server-kit"],
  ["reallyme-hephaestus-domain", "reallyme-hephaestus-contract"],
  ["reallyme-hephaestus-domain", "hephaestus-agent"],
  ["reallyme-hephaestus-contract", "hephaestus-agent"],
  ["reallyme-app-kit", "reallyme-platform"],
  ["reallyme-foundationdb-kit", "reallyme-platform"],
  ["reallyme-hephaestus-contract", "reallyme-platform"],
  ["reallyme-hephaestus-domain", "reallyme-platform"],
  ["reallyme-nats-kit", "reallyme-platform"],
  ["reallyme-postgres-kit", "reallyme-platform"],
  ["reallyme-s3-kit", "reallyme-platform"],
  ["reallyme-server-kit", "reallyme-platform"],
  ["reallyme-typesense-kit", "reallyme-platform"],
  ["reallyme-valkey-kit", "reallyme-platform"],
]);
// These three crates were published from clean commit
// 7de3d0e1b94c8553e90afe83a50b51a63f364c96 before the remaining 0.1.0 release
// was completed. Cargo embeds the packaging commit in every archive, so a later
// clean commit cannot reproduce their archive bytes.
// Pinning the immutable crates.io checksums confines source-payload comparison
// to this known partial release rather than accepting an arbitrary prior upload.
const KNOWN_PARTIAL_RELEASE_ARCHIVES = Object.freeze({
  "reallyme-app-kit@0.1.0":
    "b8ac30887a265bd250e5f59fe6f3e4326855d9745c3b64bffa53c1b563a9e39d",
  "reallyme-hephaestus-domain@0.1.0":
    "720e355cce4a6175e9bac1a75a327e96b693efc4595edadd31c4a466dee44980",
  "reallyme-server-kit@0.1.0":
    "20a28767e6aecbb865d3e8f2f69234789ec5a780b014f6828d3adc32235f87f9",
});

const arguments_ = process.argv.slice(2);
const mode = arguments_[0] ?? MODE_INSPECT;
const allowDirty = arguments_.includes("--allow-dirty");
const unknownArguments = arguments_
  .slice(1)
  .filter((argument) => argument !== "--allow-dirty");
const releaseVersion = process.env.RELEASE_VERSION ?? "";

if (
  (mode !== MODE_INSPECT && mode !== MODE_ORDER && mode !== MODE_PUBLISH) ||
  unknownArguments.length !== 0
) {
  console.error(
    `usage: node scripts/publish_crates_in_order.mjs ` +
      `${MODE_INSPECT}|${MODE_ORDER}|${MODE_PUBLISH} ` +
      "[--allow-dirty]",
  );
  process.exit(2);
}
if (allowDirty && mode === MODE_PUBLISH) {
  console.error("--allow-dirty is never supported for publication");
  process.exit(2);
}
if (mode === MODE_PUBLISH && releaseVersion.length === 0) {
  console.error("RELEASE_VERSION must be set when publishing crates");
  process.exit(2);
}
if (releaseVersion.length !== 0 && !/^[0-9]+[.][0-9]+[.][0-9]+$/u.test(releaseVersion)) {
  console.error("RELEASE_VERSION must be an exact semver release such as 0.1.0");
  process.exit(2);
}

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, {
    encoding: "utf8",
    maxBuffer: 8_388_608,
    stdio: options.capture ? "pipe" : "inherit",
  });
  if (result.error !== undefined) {
    throw result.error;
  }
  return result;
}

function verifyPublicationWorktree() {
  const statusResult = run(
    "git",
    ["status", "--porcelain=v1", "--untracked-files=all"],
    { capture: true },
  );
  if (statusResult.status !== 0) {
    process.stderr.write(statusResult.stderr);
    process.exit(statusResult.status ?? 1);
  }
  if (statusResult.stdout.length !== 0) {
    console.error("publication requires a completely clean Git worktree");
    process.exit(1);
  }

  for (const generatedDirectory of [
    "apps/example/contract/src/generated",
    "components/hephaestus/contract/src/generated",
  ]) {
    if (!fs.existsSync(generatedDirectory)) {
      console.error(`missing committed contract sources: ${generatedDirectory}`);
      process.exit(1);
    }
  }
}

if (mode === MODE_PUBLISH) {
  verifyPublicationWorktree();
}

function sleepMilliseconds(delayMilliseconds) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, delayMilliseconds);
}

function retryAfterMilliseconds(output) {
  const match = /try again after ([^\n.]+ GMT)/iu.exec(output);
  if (match === null) {
    return null;
  }
  const retryAt = Date.parse(match[1]);
  if (!Number.isFinite(retryAt)) {
    return null;
  }
  return Math.max(retryAt - Date.now() + 10_000, 10_000);
}

const metadataResult = run(
  "cargo",
  ["metadata", "--locked", "--format-version", "1", "--no-deps"],
  { capture: true },
);
if (metadataResult.status !== 0) {
  process.stderr.write(metadataResult.stderr);
  process.exit(metadataResult.status ?? 1);
}

let metadata;
try {
  metadata = JSON.parse(metadataResult.stdout);
} catch {
  console.error("cargo metadata returned invalid JSON");
  process.exit(1);
}
if (metadata === null || typeof metadata !== "object" || !Array.isArray(metadata.packages)) {
  console.error("cargo metadata did not contain a package list");
  process.exit(1);
}

const publishable = new Map();
for (const pkg of metadata.packages) {
  if (!Array.isArray(pkg.publish) || pkg.publish.length === 0) {
    continue;
  }
  if (pkg.publish.length !== 1 || pkg.publish[0] !== "crates-io") {
    console.error(`${pkg.name} must publish only to crates.io`);
    process.exit(1);
  }
  if (
    typeof pkg.description !== "string" ||
    pkg.description.length === 0 ||
    pkg.repository !== "https://github.com/reallyme/platform" ||
    pkg.license !== "MIT OR Apache-2.0"
  ) {
    console.error(`${pkg.name} is missing required public release metadata`);
    process.exit(1);
  }
  publishable.set(pkg.name, pkg);
}

const actualPublishableNames = [...publishable.keys()].sort();
if (
  actualPublishableNames.length !== EXPECTED_PUBLISHABLE_PACKAGES.length ||
  actualPublishableNames.some(
    (packageName, index) => packageName !== EXPECTED_PUBLISHABLE_PACKAGES[index],
  )
) {
  console.error(`unexpected publishable package set: ${actualPublishableNames.join(", ")}`);
  process.exit(1);
}

function dependencyPackageName(dependency) {
  return dependency.package ?? dependency.name;
}

function isPublishOrderingDependency(dependency) {
  return (
    dependency.source === null &&
    typeof dependency.path === "string" &&
    dependency.kind !== "dev" &&
    publishable.has(dependencyPackageName(dependency))
  );
}

function parseVersion(version) {
  if (!/^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$/u.test(version)) {
    return null;
  }
  const parts = version.split(".").map((part) => Number.parseInt(part, 10));
  if (parts.some((part) => !Number.isSafeInteger(part) || part < 0)) {
    return null;
  }
  return { major: parts[0], minor: parts[1], patch: parts[2] };
}

function isRequirementSatisfied(requirement, version) {
  if (!requirement.startsWith("^")) {
    return requirement === `=${version}` || requirement === version;
  }
  const minimum = parseVersion(requirement.slice(1));
  const actual = parseVersion(version);
  if (minimum === null || actual === null || actual.major !== minimum.major) {
    return false;
  }
  if (minimum.major === 0 && actual.minor !== minimum.minor) {
    return false;
  }
  if (minimum.major === 0 && minimum.minor === 0 && actual.patch !== minimum.patch) {
    return false;
  }
  return (
    actual.minor > minimum.minor ||
    (actual.minor === minimum.minor && actual.patch >= minimum.patch)
  );
}

for (const pkg of publishable.values()) {
  for (const dependency of pkg.dependencies) {
    if (!isPublishOrderingDependency(dependency)) {
      continue;
    }
    const dependencyPackage = publishable.get(dependencyPackageName(dependency));
    if (!isRequirementSatisfied(dependency.req, dependencyPackage.version)) {
      console.error(
        `${pkg.name} depends on ${dependency.name} ${dependency.req}; ` +
          `local version is ${dependencyPackage.version}`,
      );
      process.exit(1);
    }
  }
}

const visiting = new Set();
const visited = new Set();
const ordered = [];
function visit(pkg) {
  if (visited.has(pkg.name)) {
    return;
  }
  if (visiting.has(pkg.name)) {
    console.error(`workspace publish dependency cycle at ${pkg.name}`);
    process.exit(1);
  }
  visiting.add(pkg.name);
  for (const dependency of pkg.dependencies) {
    const dependencyName = dependencyPackageName(dependency);
    if (isPublishOrderingDependency(dependency)) {
      visit(publishable.get(dependencyName));
    }
  }
  visiting.delete(pkg.name);
  visited.add(pkg.name);
  ordered.push(pkg);
}
for (const pkg of publishable.values()) {
  visit(pkg);
}

const orderedIndexByName = new Map();
ordered.forEach((pkg, index) => orderedIndexByName.set(pkg.name, index));
for (const [dependencyName, packageName] of REQUIRED_PUBLISH_ORDER_EDGES) {
  const dependencyIndex = orderedIndexByName.get(dependencyName);
  const packageIndex = orderedIndexByName.get(packageName);
  if (
    dependencyIndex === undefined ||
    packageIndex === undefined ||
    dependencyIndex >= packageIndex
  ) {
    console.error(`${dependencyName} must publish before ${packageName}`);
    process.exit(1);
  }
}
if (
  releaseVersion.length !== 0 &&
  ordered.some((pkg) => pkg.version !== releaseVersion)
) {
  console.error(`all public crates must have release version ${releaseVersion}`);
  process.exit(1);
}

console.log(`Publish order (${ordered.length} crates):`);
for (const pkg of ordered) {
  console.log(`- ${pkg.name} ${pkg.version}`);
}
if (mode === MODE_ORDER) {
  process.exit(0);
}

const packageDirectory = path.join(metadata.target_directory, "package");
const unpackDirectory = path.join(packageDirectory, "release-preflight");

function packageWorkspaceForInspection() {
  const packageArguments = ["package"];
  for (const pkg of ordered) {
    packageArguments.push("--package", pkg.name);
  }
  packageArguments.push("--no-verify", "--locked");
  if (allowDirty) {
    packageArguments.push("--allow-dirty");
  }
  const packageResult = run("cargo", packageArguments);
  if (packageResult.status !== 0) {
    process.exit(packageResult.status ?? 1);
  }
  fs.rmSync(unpackDirectory, { force: true, recursive: true });
  fs.mkdirSync(unpackDirectory, { recursive: true });
  for (const pkg of ordered) {
    const archive = path.join(packageDirectory, `${pkg.name}-${pkg.version}.crate`);
    if (!fs.existsSync(archive)) {
      console.error(`missing release archive: ${archive}`);
      process.exit(1);
    }
    const extractResult = run("tar", ["-xzf", archive, "-C", unpackDirectory]);
    if (extractResult.status !== 0) {
      process.exit(extractResult.status ?? 1);
    }
  }
}

function unresolvedRegistryPackages(output) {
  const missing = [];
  const noMatchPattern = /no matching package named `([^`]+)` found/gu;
  for (let match = noMatchPattern.exec(output); match !== null; match = noMatchPattern.exec(output)) {
    missing.push(match[1]);
  }
  const versionPattern = /failed to select a version for the requirement `([^`\s]+) =/gu;
  for (let match = versionPattern.exec(output); match !== null; match = versionPattern.exec(output)) {
    missing.push(match[1]);
  }
  return [...new Set(missing)];
}

function isEarlierWorkspaceDependency(pkg, dependencyName) {
  const packageIndex = orderedIndexByName.get(pkg.name);
  const dependencyIndex = orderedIndexByName.get(dependencyName);
  return (
    dependencyIndex !== undefined &&
    packageIndex !== undefined &&
    dependencyIndex < packageIndex
  );
}

function patchArgumentsFor(pkg) {
  const requiredWorkspaceDependencies = new Set();
  const collectWorkspaceDependencies = (currentPackage) => {
    for (const dependency of currentPackage.dependencies) {
      if (!isPublishOrderingDependency(dependency)) {
        continue;
      }
      const dependencyName = dependencyPackageName(dependency);
      if (requiredWorkspaceDependencies.has(dependencyName)) {
        continue;
      }
      requiredWorkspaceDependencies.add(dependencyName);
      collectWorkspaceDependencies(publishable.get(dependencyName));
    }
  };
  collectWorkspaceDependencies(pkg);

  const patchArguments = [];
  for (const dependencyPackage of ordered) {
    if (!requiredWorkspaceDependencies.has(dependencyPackage.name)) {
      continue;
    }
    const dependencyPath = path.join(
      unpackDirectory,
      `${dependencyPackage.name}-${dependencyPackage.version}`,
    );
    patchArguments.push(
      "--config",
      `patch.crates-io.'${dependencyPackage.name}'.path=${JSON.stringify(dependencyPath)}`,
    );
  }
  return patchArguments;
}

function inspectPackage(pkg) {
  const listArguments = ["package", "-p", pkg.name, "--list", "--locked"];
  if (allowDirty) {
    listArguments.push("--allow-dirty");
  }
  const listResult = run("cargo", listArguments);
  if (listResult.status !== 0) {
    process.exit(listResult.status ?? 1);
  }

  const manifestPath = path.join(unpackDirectory, `${pkg.name}-${pkg.version}`, "Cargo.toml");
  const patchArguments = patchArgumentsFor(pkg);
  const fetchArguments = ["fetch", "--manifest-path", manifestPath, ...patchArguments];
  if (patchArguments.length === 0) {
    fetchArguments.push("--locked");
  }
  const fetchResult = run("cargo", fetchArguments);
  if (fetchResult.status !== 0) {
    process.exit(fetchResult.status ?? 1);
  }
  const checkResult = run("cargo", [
    "check",
    "--manifest-path",
    manifestPath,
    "--all-features",
    "--locked",
    "--offline",
    ...patchArguments,
  ]);
  if (checkResult.status !== 0) {
    process.exit(checkResult.status ?? 1);
  }

  const dryRunArguments = ["publish", "-p", pkg.name, "--dry-run", "--locked"];
  if (allowDirty) {
    dryRunArguments.push("--allow-dirty");
  }
  const dryRunResult = run("cargo", dryRunArguments, { capture: true });
  process.stdout.write(dryRunResult.stdout);
  process.stderr.write(dryRunResult.stderr);
  if (dryRunResult.status === 0) {
    return;
  }
  const missing = unresolvedRegistryPackages(`${dryRunResult.stdout}\n${dryRunResult.stderr}`);
  if (
    missing.length !== 0 &&
    missing.every((dependencyName) => isEarlierWorkspaceDependency(pkg, dependencyName))
  ) {
    console.log(
      `${pkg.name} dry-run reached unpublished ordered dependencies: ${missing.join(", ")}`,
    );
    return;
  }
  process.exit(dryRunResult.status ?? 1);
}

function verifyPublishedPackageMatches(pkg) {
  const localArchive = path.join(packageDirectory, `${pkg.name}-${pkg.version}.crate`);
  if (!fs.existsSync(localArchive)) {
    console.error(`${pkg.name} ${pkg.version} local package archive is missing`);
    process.exit(1);
  }

  const comparisonDirectory = fs.mkdtempSync(path.join(packageDirectory, "published-"));
  const publishedArchive = path.join(comparisonDirectory, `${pkg.name}-${pkg.version}.crate`);
  const encodedName = encodeURIComponent(pkg.name);
  const encodedVersion = encodeURIComponent(pkg.version);
  const downloadUrl =
    `https://static.crates.io/crates/${encodedName}/` +
    `${encodedName}-${encodedVersion}.crate`;

  try {
    const downloadResult = run(
      "curl",
      [
        "--fail-with-body",
        "--location",
        "--proto",
        "=https",
        "--tlsv1.2",
        "--retry",
        "5",
        "--retry-all-errors",
        "--output",
        publishedArchive,
        downloadUrl,
      ],
      { capture: true },
    );
    if (downloadResult.status !== 0) {
      process.stdout.write(downloadResult.stdout);
      process.stderr.write(downloadResult.stderr);
      process.exit(downloadResult.status ?? 1);
    }

    const localChecksum = createHash("sha256").update(fs.readFileSync(localArchive)).digest("hex");
    const publishedChecksum = createHash("sha256")
      .update(fs.readFileSync(publishedArchive))
      .digest("hex");
    if (localChecksum === publishedChecksum) {
      return;
    }

    const partialReleaseKey = `${pkg.name}@${pkg.version}`;
    if (KNOWN_PARTIAL_RELEASE_ARCHIVES[partialReleaseKey] !== publishedChecksum) {
      console.error(`${pkg.name} ${pkg.version} is already published from different source bytes`);
      process.exit(1);
    }

    try {
      compareCratePayloads({
        localArchive,
        publishedArchive,
        packageName: pkg.name,
        packageVersion: pkg.version,
        workspacePackageNames: actualPublishableNames,
        temporaryDirectory: comparisonDirectory,
      });
    } catch (error) {
      const reason = error instanceof CratePayloadError ? error.code : "comparison-failed";
      console.error(
        `${pkg.name} ${pkg.version} published source does not match the certified release (${reason})`,
      );
      process.exit(1);
    }
    console.log(
      `${pkg.name} ${pkg.version} matches the known clean partial-release source payload`,
    );
  } finally {
    fs.rmSync(comparisonDirectory, { force: true, recursive: true });
  }
}

function publishPackage(pkg) {
  const packageResult = run("cargo", [
    "package",
    "-p",
    pkg.name,
    "--no-verify",
    "--locked",
  ]);
  if (packageResult.status !== 0) {
    process.exit(packageResult.status ?? 1);
  }
  for (let attempt = 1; attempt <= MAX_PUBLISH_ATTEMPTS; attempt += 1) {
    const result = run(
      "cargo",
      ["publish", "-p", pkg.name, "--locked"],
      { capture: true },
    );
    process.stdout.write(result.stdout);
    process.stderr.write(result.stderr);
    if (result.status === 0) {
      return;
    }
    const combined = `${result.stdout}\n${result.stderr}`;
    if (combined.includes("already uploaded") || combined.includes("already exists")) {
      verifyPublishedPackageMatches(pkg);
      console.log(`${pkg.name} ${pkg.version} is already published and matches; continuing`);
      return;
    }
    const lowerCombined = combined.toLowerCase();
    const rateLimited =
      lowerCombined.includes("too many requests") ||
      lowerCombined.includes("rate-limited") ||
      lowerCombined.includes("rate limited");
    if (rateLimited && attempt < MAX_PUBLISH_ATTEMPTS) {
      const delay = retryAfterMilliseconds(combined) ?? CRATES_IO_DEFAULT_RATE_LIMIT_RETRY_MS;
      console.log(`crates.io rate limit: retrying ${pkg.name} in ${Math.ceil(delay / 1_000)}s`);
      sleepMilliseconds(delay);
      continue;
    }
    if (combined.includes("no matching package named") && attempt < MAX_PUBLISH_ATTEMPTS) {
      const delay = attempt * CRATES_IO_INDEX_RETRY_BASE_MS;
      console.log(`crates.io index delay: retrying ${pkg.name} in ${delay / 1_000}s`);
      sleepMilliseconds(delay);
      continue;
    }
    process.exit(result.status ?? 1);
  }
}

if (mode === MODE_INSPECT) {
  packageWorkspaceForInspection();
}
for (const pkg of ordered) {
  if (mode === MODE_INSPECT) {
    inspectPackage(pkg);
  } else {
    publishPackage(pkg);
  }
}
