#!/usr/bin/env node
// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";

export class CratePayloadError extends Error {
  constructor(code) {
    super(code);
    this.name = "CratePayloadError";
    this.code = code;
  }
}

function runTar(arguments_) {
  const result = spawnSync("tar", arguments_, {
    encoding: "utf8",
    maxBuffer: 8_388_608,
    stdio: "pipe",
  });
  if (result.error !== undefined || result.status !== 0) {
    throw new CratePayloadError("archive-read-failed");
  }
  return result.stdout;
}

function validateArchiveEntries(archive, packageRoot) {
  const listing = runTar(["-tzf", archive]);
  const entries = listing.split("\n").filter((entry) => entry.length !== 0);
  if (entries.length === 0) {
    throw new CratePayloadError("empty-archive");
  }

  for (const listedEntry of entries) {
    const entry = listedEntry.endsWith("/") ? listedEntry.slice(0, -1) : listedEntry;
    if (entry === packageRoot) {
      continue;
    }
    if (!entry.startsWith(`${packageRoot}/`)) {
      throw new CratePayloadError("archive-entry-outside-package-root");
    }
    const relativePath = entry.slice(packageRoot.length + 1);
    const segments = relativePath.split("/");
    if (segments.some((segment) => segment.length === 0 || segment === "." || segment === "..")) {
      throw new CratePayloadError("invalid-archive-entry-path");
    }
  }
}

function extractArchive(archive, destination, packageRoot) {
  validateArchiveEntries(archive, packageRoot);
  fs.mkdirSync(destination, { recursive: true, mode: 0o700 });
  runTar(["-xzf", archive, "--no-same-owner", "-C", destination]);
}

function normalizeCargoLock(contents, workspacePackageNames, packageVersion) {
  const workspaceNames = new Set(workspacePackageNames);
  const text = contents.toString("utf8");
  const blocks = text.split(/(?=^\[\[package\]\]$)/mu);
  return Buffer.from(
    blocks
      .map((block) => {
        const name = /^name = "([^"]+)"$/mu.exec(block)?.[1];
        const version = /^version = "([^"]+)"$/mu.exec(block)?.[1];
        if (name === undefined || version !== packageVersion || !workspaceNames.has(name)) {
          return block;
        }
        return block.replace(
          /^checksum = "[0-9a-f]{64}"$/mu,
          'checksum = "<same-release-workspace-crate>"',
        );
      })
      .join(""),
    "utf8",
  );
}

function collectPayload(packageDirectory, workspacePackageNames, packageVersion) {
  const payload = new Map();

  function visit(directory, relativeDirectory) {
    const entries = fs.readdirSync(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const relativePath =
        relativeDirectory.length === 0
          ? entry.name
          : path.posix.join(relativeDirectory, entry.name);
      const absolutePath = path.join(directory, entry.name);
      const status = fs.lstatSync(absolutePath);
      if (status.isSymbolicLink()) {
        throw new CratePayloadError("archive-symlink-not-supported");
      }
      if (status.isDirectory()) {
        visit(absolutePath, relativePath);
        continue;
      }
      if (!status.isFile()) {
        throw new CratePayloadError("archive-special-file-not-supported");
      }
      if (relativePath === ".cargo_vcs_info.json") {
        continue;
      }

      const rawContents = fs.readFileSync(absolutePath);
      const contents =
        relativePath === "Cargo.lock"
          ? normalizeCargoLock(rawContents, workspacePackageNames, packageVersion)
          : rawContents;
      payload.set(relativePath, {
        contents,
        executable: (status.mode & 0o111) !== 0,
      });
    }
  }

  visit(packageDirectory, "");
  return payload;
}

export function compareCratePayloads({
  localArchive,
  publishedArchive,
  packageName,
  packageVersion,
  workspacePackageNames,
  temporaryDirectory,
}) {
  const packageRoot = `${packageName}-${packageVersion}`;
  const comparisonDirectory = fs.mkdtempSync(path.join(temporaryDirectory, "payload-"));
  const localDirectory = path.join(comparisonDirectory, "local");
  const publishedDirectory = path.join(comparisonDirectory, "published");

  try {
    extractArchive(localArchive, localDirectory, packageRoot);
    extractArchive(publishedArchive, publishedDirectory, packageRoot);
    const localPayload = collectPayload(
      path.join(localDirectory, packageRoot),
      workspacePackageNames,
      packageVersion,
    );
    const publishedPayload = collectPayload(
      path.join(publishedDirectory, packageRoot),
      workspacePackageNames,
      packageVersion,
    );

    if (localPayload.size !== publishedPayload.size) {
      throw new CratePayloadError("archive-payload-mismatch");
    }
    for (const [relativePath, localFile] of localPayload) {
      const publishedFile = publishedPayload.get(relativePath);
      if (
        publishedFile === undefined ||
        localFile.executable !== publishedFile.executable ||
        !localFile.contents.equals(publishedFile.contents)
      ) {
        throw new CratePayloadError("archive-payload-mismatch");
      }
    }
  } finally {
    fs.rmSync(comparisonDirectory, { force: true, recursive: true });
  }
}
