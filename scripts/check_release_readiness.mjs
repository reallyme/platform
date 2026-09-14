#!/usr/bin/env node
// SPDX-FileCopyrightText: 2026 ReallyMe LLC
// SPDX-License-Identifier: MIT OR Apache-2.0

import { existsSync } from "node:fs";

import { createReleaseReadinessContext } from "./release-readiness/core.mjs";

const requireTrackedFiles =
  process.env.REALLYME_RELEASE_READINESS_REQUIRE_TRACKED === "1";
const context = createReleaseReadinessContext({
  scriptUrl: import.meta.url,
  requireTrackedFiles,
});

const {
  fail,
  assertCargoWorkspacePolicy,
  assertContains,
  assertNodeWorkflowJobsPinNode,
  assertRepositoryShapePolicy,
  assertReallyMeVendoredCorePolicy,
  assertRustSourcePolicy,
  assertSpdxHeaders,
  assertTextPolicy,
  assertWorkflowActionsPinned,
  loadTrackedFiles,
  readText,
} = context;

if (requireTrackedFiles) {
  assertReallyMeVendoredCorePolicy({
    scriptPath: "scripts/check_release_readiness.mjs",
    corePath: "scripts/release-readiness/core.mjs",
    version: "0.6.2",
  });
}

assertContains(
  "scripts/release-readiness/core.mjs",
  'RELEASE_READINESS_VERSION = "0.6.2"',
);
assertWorkflowActionsPinned();
assertNodeWorkflowJobsPinNode({ nodeVersion: "24" });
assertCargoWorkspacePolicy({
  requireWorkspaceLints: true,
  requirePublishInclude: true,
  validatePublishablePathDependencies: true,
});
assertRustSourcePolicy({
  roots: ["."],
  generatedPrefixes: ["apps/example/contract/src/generated"],
  baselinePath: null,
  productionTargetLines: 500,
  productionHardLines: 500,
  testTargetLines: 800,
  testHardLines: 800,
  moduleHardLines: 200,
  forbidWildcardImports: true,
  forbidInlineTests: true,
  forbidSubstantiveFacades: true,
  forbidPanickingProductionCode: true,
  forbidDynamicErrorSurfaces: true,
});
assertSpdxHeaders({
  copyright: "SPDX-FileCopyrightText: 2026 ReallyMe LLC",
  license: "SPDX-License-Identifier: MIT OR Apache-2.0",
  exclusions: [
    {
      path: "apps/example/contract/src/generated",
      reason: "generated",
    },
    {
      path: "scripts/release-readiness/core.mjs",
      reason: "vendored",
    },
  ],
  requireExclusionsMatched: true,
  requireExclusionReasons: true,
});
for (const path of loadTrackedFiles()) {
  if (
    (!path.endsWith(".md") && !path.endsWith(".txt")) ||
    !existsSync(path)
  ) {
    continue;
  }
  const text = readText(path);
  if (text.includes("SPDX-FileCopyrightText:") || text.includes("SPDX-License-Identifier:")) {
    fail(`${path} must not contain SPDX headers`);
  }
}
assertContains("LICENSE-MIT", "MIT License");
assertContains("LICENSE-APACHE", "Apache License");
assertRepositoryShapePolicy({
  archetype: "platform-workspace",
  requiredLanes: [
    "crates",
    "kits",
    "apps",
    "servers",
    "workers",
    "conformance",
    "docs",
    "scripts",
    ".github",
  ],
  optionalLanes: [],
  exceptions: [],
  crates: [{ path: "crates/platform", role: "facade" }],
  subLanes: {
    apps: ["example"],
    kits: [
      "app",
      "foundationdb",
      "nats",
      "postgres",
      "s3",
      "server",
      "typesense",
      "valkey",
    ],
    servers: ["example"],
    workers: ["example"],
  },
  forbiddenPaths: [
    "src",
    "components",
    "apps/reallyme-domain",
    "kits/reallyme-app-kit",
    "kits/reallyme-server-kit",
    "kits/reallyme-foundationdb-kit",
    "kits/reallyme-nats-kit",
    "kits/reallyme-postgres-kit",
    "kits/reallyme-s3-kit",
    "kits/reallyme-typesense-kit",
    "kits/reallyme-valkey-kit",
    "servers/example-server",
    "workers/example-worker",
  ],
  requireReleaseReadiness: true,
});
assertTextPolicy({
  files: [
    {
      path: "Cargo.toml",
      required: [
        "[workspace]",
        'license = "MIT OR Apache-2.0"',
        "overflow-checks = true",
      ],
      forbidden: ["[package]", "[patch.crates-io]"],
    },
    {
      path: "crates/platform/Cargo.toml",
      required: ['default = []', 'publish = ["crates-io"]'],
      forbidden: ['default = ["native-server"]'],
    },
  ],
});
