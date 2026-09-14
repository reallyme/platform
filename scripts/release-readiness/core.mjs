// SPDX-FileCopyrightText: 2026 ReallyMe LLC
//
// SPDX-License-Identifier: MIT OR Apache-2.0

import { lstatSync, readdirSync, readFileSync, realpathSync, writeSync } from "node:fs";
import { createHash } from "node:crypto";
import { extname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

// This module is intentionally written as a standalone, vendorable release
// readiness core. Sister repositories should copy it byte-for-byte or consume a
// pinned upstream revision so release-critical checks do not drift silently.
export const RELEASE_READINESS_VERSION = "0.6.2";

const DEFAULT_FAILURE_PREFIX = "release readiness check failed";
const MAX_PRODUCTION_SOURCE_LINES = 500;
const MAX_SEPARATE_TEST_SOURCE_LINES = 800;
const DEFAULT_REALLYME_LATEST_STABLE_DEPENDENCIES = [
  "reallyme-crypto",
  "reallyme-codec",
  "reallyme-jose",
  "reallyme-cose",
];

const escapeRegExpLiteral = (value) => value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");

const scrubProtoCommentsAndStrings = (source) => {
  let output = "";
  let state = "normal";
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (state === "normal") {
      if (character === "/" && next === "/") {
        output += "  ";
        index += 1;
        state = "line-comment";
      } else if (character === "/" && next === "*") {
        output += "  ";
        index += 1;
        state = "block-comment";
      } else if (character === '"' || character === "'") {
        output += " ";
        state = character === '"' ? "double-quoted-string" : "single-quoted-string";
      } else {
        output += character;
      }
      continue;
    }
    if (state === "line-comment") {
      if (character === "\n") {
        output += "\n";
        state = "normal";
      } else {
        output += " ";
      }
      continue;
    }
    if (state === "block-comment") {
      if (character === "*" && next === "/") {
        output += "  ";
        index += 1;
        state = "normal";
      } else {
        output += character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (character === "\\" && next !== undefined) {
      output += next === "\n" ? " \n" : "  ";
      index += 1;
    } else if (
      (state === "double-quoted-string" && character === '"') ||
      (state === "single-quoted-string" && character === "'")
    ) {
      output += " ";
      state = "normal";
    } else {
      output += character === "\n" ? "\n" : " ";
    }
  }
  return output;
};

const javascriptSlashStartsRegex = (source, index) => {
  let cursor = index - 1;
  while (cursor >= 0 && /\s/u.test(source[cursor])) {
    cursor -= 1;
  }
  if (cursor < 0) {
    return true;
  }
  return "=(:,[!&|?;{}".includes(source[cursor]);
};

const scrubJavaScriptCommentsAndStrings = (source, options = {}) => {
  const preserveStrings = options.preserveStrings ?? false;
  const preserveComments = options.preserveComments ?? false;
  let output = "";
  let state = "normal";
  let regexCharacterClass = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (state === "normal") {
      if (character === "/" && next === "/") {
        output += preserveComments ? "//" : "  ";
        index += 1;
        state = "line-comment";
      } else if (character === "/" && next === "*") {
        output += preserveComments ? "/*" : "  ";
        index += 1;
        state = "block-comment";
      } else if (character === "/" && javascriptSlashStartsRegex(source, index)) {
        output += preserveStrings ? character : " ";
        regexCharacterClass = false;
        state = "regex";
      } else if (character === '"' || character === "'" || character === "`") {
        output += preserveStrings ? character : " ";
        state =
          character === '"'
            ? "double-quoted-string"
            : character === "'"
              ? "single-quoted-string"
              : "template-string";
      } else {
        output += character;
      }
      continue;
    }
    if (state === "line-comment") {
      if (character === "\n") {
        output += "\n";
        state = "normal";
      } else {
        output += preserveComments ? character : " ";
      }
      continue;
    }
    if (state === "block-comment") {
      if (character === "*" && next === "/") {
        output += preserveComments ? "*/" : "  ";
        index += 1;
        state = "normal";
      } else {
        output += preserveComments ? character : character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (state === "regex") {
      if (character === "\\" && next !== undefined) {
        output += preserveStrings ? `${character}${next}` : next === "\n" ? " \n" : "  ";
        index += 1;
      } else if (character === "[") {
        output += preserveStrings ? character : " ";
        regexCharacterClass = true;
      } else if (character === "]") {
        output += preserveStrings ? character : " ";
        regexCharacterClass = false;
      } else if (character === "/" && !regexCharacterClass) {
        output += preserveStrings ? character : " ";
        state = "normal";
      } else {
        output += preserveStrings ? character : character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (character === "\\" && next !== undefined) {
      output += preserveStrings ? `${character}${next}` : next === "\n" ? " \n" : "  ";
      index += 1;
    } else if (
      (state === "double-quoted-string" && character === '"') ||
      (state === "single-quoted-string" && character === "'") ||
      (state === "template-string" && character === "`")
    ) {
      output += preserveStrings ? character : " ";
      state = "normal";
    } else {
      output += preserveStrings ? character : character === "\n" ? "\n" : " ";
    }
  }
  return output;
};

const rustRawStringStart = (source, index) => {
  let cursor = index;
  if (source[cursor] === "b" && source[cursor + 1] === "r") {
    cursor += 2;
  } else if (source[cursor] === "r") {
    cursor += 1;
  } else {
    return null;
  }
  let hashCount = 0;
  while (source[cursor] === "#") {
    hashCount += 1;
    cursor += 1;
  }
  if (source[cursor] !== '"') {
    return null;
  }
  return {
    length: cursor - index + 1,
    terminator: `"${"#".repeat(hashCount)}`,
  };
};

const scrubRustCommentsAndStrings = (source) => {
  let output = "";
  let state = "normal";
  let blockDepth = 0;
  let rawTerminator = "";
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (state === "normal") {
      if (character === "/" && next === "/") {
        output += "  ";
        index += 1;
        state = "line-comment";
      } else if (character === "/" && next === "*") {
        output += "  ";
        index += 1;
        blockDepth = 1;
        state = "block-comment";
      } else {
        const rawStart = rustRawStringStart(source, index);
        if (rawStart !== null) {
          output += " ".repeat(rawStart.length);
          index += rawStart.length - 1;
          rawTerminator = rawStart.terminator;
          state = "raw-string";
        } else if (character === '"') {
          output += " ";
          state = "quoted-string";
        } else {
          output += character;
        }
      }
      continue;
    }
    if (state === "line-comment") {
      if (character === "\n") {
        output += "\n";
        state = "normal";
      } else {
        output += " ";
      }
      continue;
    }
    if (state === "block-comment") {
      if (character === "/" && next === "*") {
        output += "  ";
        index += 1;
        blockDepth += 1;
      } else if (character === "*" && next === "/") {
        output += "  ";
        index += 1;
        blockDepth -= 1;
        if (blockDepth === 0) {
          state = "normal";
        }
      } else {
        output += character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (state === "quoted-string") {
      if (character === "\\" && next !== undefined) {
        output += next === "\n" ? " \n" : "  ";
        index += 1;
      } else if (character === '"') {
        output += " ";
        state = "normal";
      } else {
        output += character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (source.startsWith(rawTerminator, index)) {
      output += " ".repeat(rawTerminator.length);
      index += rawTerminator.length - 1;
      state = "normal";
    } else {
      output += character === "\n" ? "\n" : " ";
    }
  }
  return output;
};

const scrubSlashCommentsAndStrings = (source, options = {}) => {
  const preserveComments = options.preserveComments ?? false;
  let output = "";
  let state = "normal";
  let blockDepth = 0;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (state === "normal") {
      if (character === "/" && next === "/") {
        output += preserveComments ? "//" : "  ";
        index += 1;
        state = "line-comment";
      } else if (character === "/" && next === "*") {
        output += preserveComments ? "/*" : "  ";
        index += 1;
        blockDepth = 1;
        state = "block-comment";
      } else if (source.startsWith('"""', index)) {
        output += "   ";
        index += 2;
        state = "triple-quoted-string";
      } else if (character === '"' || character === "'") {
        output += " ";
        state = character === '"' ? "double-quoted-string" : "single-quoted-string";
      } else {
        output += character;
      }
      continue;
    }
    if (state === "line-comment") {
      if (character === "\n") {
        output += "\n";
        state = "normal";
      } else {
        output += preserveComments ? character : " ";
      }
      continue;
    }
    if (state === "block-comment") {
      if (character === "/" && next === "*") {
        output += preserveComments ? "/*" : "  ";
        index += 1;
        blockDepth += 1;
      } else if (character === "*" && next === "/") {
        output += preserveComments ? "*/" : "  ";
        index += 1;
        blockDepth -= 1;
        if (blockDepth === 0) {
          state = "normal";
        }
      } else {
        output += preserveComments ? character : character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (state === "triple-quoted-string") {
      if (source.startsWith('"""', index)) {
        output += "   ";
        index += 2;
        state = "normal";
      } else {
        output += character === "\n" ? "\n" : " ";
      }
      continue;
    }
    if (character === "\\" && next !== undefined) {
      output += next === "\n" ? " \n" : "  ";
      index += 1;
    } else if (
      (state === "double-quoted-string" && character === '"') ||
      (state === "single-quoted-string" && character === "'")
    ) {
      output += " ";
      state = "normal";
    } else {
      output += character === "\n" ? "\n" : " ";
    }
  }
  return output;
};

export function createReleaseReadinessContext(options) {
  const {
    scriptUrl,
    repoRoot = "..",
    requireTrackedFiles = false,
    failurePrefix = DEFAULT_FAILURE_PREFIX,
  } = options ?? {};

  if (typeof scriptUrl !== "string" || scriptUrl.length === 0) {
    console.error(`${failurePrefix}: scriptUrl is required`);
    process.exit(1);
  }

  let root;
  try {
    // Canonicalize the repository root so containment checks remain stable when
    // the caller reached the worktree through an operating-system path alias
    // such as /tmp versus /private/tmp.
    root = realpathSync(resolve(fileURLToPath(new URL(repoRoot, scriptUrl))));
  } catch {
    console.error(`${failurePrefix}: repository root is missing or inaccessible`);
    process.exit(1);
  }
  let trackedFiles = null;

  const fail = (message) => {
    console.error(`${failurePrefix}: ${message}`);
    process.exit(1);
  };

  const reportSourcePolicy = (language) => {
    if (
      process.env.RELEASE_READINESS_ENFORCED_VERSION !== RELEASE_READINESS_VERSION ||
      process.env.RELEASE_READINESS_SOURCE_POLICY_FD !== "3"
    ) {
      return;
    }
    try {
      writeSync(3, `${language}\n`);
    } catch {
      fail(`could not report the enforced ${language} source policy`);
    }
  };

  const resolveRepositoryPath = (path, description = "path") => {
    if (
      typeof path !== "string" ||
      path.length === 0 ||
      path.includes("\0") ||
      isAbsolute(path)
    ) {
      fail(`${description} must be a non-empty repository-relative path`);
    }
    const absolute = resolve(root, path);
    const repositoryRelative = relative(root, absolute);
    if (
      repositoryRelative === ".." ||
      repositoryRelative.startsWith(`..${sep}`) ||
      isAbsolute(repositoryRelative)
    ) {
      fail(`${description} escapes the repository root`);
    }
    return absolute;
  };

  const assertCanonicalPathInsideRepository = (absolute, description) => {
    let canonical;
    try {
      canonical = realpathSync(absolute);
    } catch {
      fail(`${description} is missing or inaccessible`);
    }
    const repositoryRelative = relative(root, canonical);
    if (
      repositoryRelative === ".." ||
      repositoryRelative.startsWith(`..${sep}`) ||
      isAbsolute(repositoryRelative)
    ) {
      fail(`${description} resolves outside the repository root`);
    }
    return canonical;
  };

  const assertRegularFile = (path) => {
    const absolute = resolveRepositoryPath(path);
    let status;
    try {
      status = lstatSync(absolute);
    } catch {
      fail(`${path} is missing from the worktree`);
    }
    if (status.isSymbolicLink()) {
      fail(`${path} must not be a symbolic link`);
    }
    if (!status.isFile()) {
      fail(`${path} is not a regular file`);
    }
    return assertCanonicalPathInsideRepository(absolute, path);
  };

  const assertRepositoryDirectory = (path, description = path) => {
    const absolute = resolveRepositoryPath(path, description);
    let status;
    try {
      status = lstatSync(absolute);
    } catch {
      fail(`${description} is missing from the worktree`);
    }
    if (status.isSymbolicLink()) {
      fail(`${description} must not be a symbolic link`);
    }
    if (!status.isDirectory()) {
      fail(`${description} is not a directory`);
    }
    return assertCanonicalPathInsideRepository(absolute, description);
  };

  const run = (command, args, runOptions = {}) => {
    if (typeof command !== "string" || command.length === 0) {
      fail("release readiness command must be a non-empty string");
    }
    if (!Array.isArray(args) || args.some((arg) => typeof arg !== "string")) {
      fail(`${command} arguments must be an array of strings`);
    }
    const cwd =
      runOptions.cwd === undefined
        ? root
        : assertRepositoryDirectory(
            runOptions.cwd,
            `${command} working directory`,
          );
    const result = spawnSync(command, args, {
      cwd,
      encoding: "utf8",
      stdio: runOptions.capture ? "pipe" : "inherit",
      env: runOptions.env ?? process.env,
    });
    if (result.error) {
      fail(`${[command, ...args].join(" ")} failed to start: ${result.error.message}`);
    }
    if (result.status !== 0) {
      if (runOptions.capture) {
        process.stdout.write(result.stdout ?? "");
        process.stderr.write(result.stderr ?? "");
      }
      process.exit(result.status ?? 1);
    }
    return result;
  };

  const loadTrackedFiles = () => {
    if (trackedFiles !== null) {
      return trackedFiles;
    }
    const result = spawnSync("git", ["ls-files", "-z"], {
      cwd: root,
      encoding: "utf8",
      stdio: "pipe",
    });
    if (result.error) {
      fail(`git ls-files -z failed to start: ${result.error.message}`);
    }
    if (result.status !== 0) {
      process.stdout.write(result.stdout ?? "");
      process.stderr.write(result.stderr ?? "");
      process.exit(result.status ?? 1);
    }
    trackedFiles = new Set(result.stdout.split("\0").filter(Boolean));
    return trackedFiles;
  };

  const requireTracked = (path) => {
    resolveRepositoryPath(path);
    if (!loadTrackedFiles().has(path)) {
      fail(`${path} is not tracked by Git`);
    }
  };

  const assertPathsAbsent = (paths) => {
    if (
      !Array.isArray(paths) ||
      paths.some((path) => typeof path !== "string" || path.length === 0)
    ) {
      fail("absent path policy must be an array of non-empty strings");
    }
    for (const path of paths) {
      const absolute = resolveRepositoryPath(path);
      try {
        // lstat observes broken symlinks as well as live files and directories.
        // existsSync would silently treat a broken compatibility symlink as
        // absent, leaving a retired entry in release artifacts.
        lstatSync(absolute);
        fail(`${path} must not exist`);
      } catch (error) {
        if (
          error === null ||
          typeof error !== "object" ||
          !("code" in error) ||
          error.code !== "ENOENT"
        ) {
          fail(`${path} absence could not be verified`);
        }
      }
      if (
        requireTrackedFiles &&
        [...loadTrackedFiles()].some(
          (trackedPath) => trackedPath === path || trackedPath.startsWith(`${path}/`),
        )
      ) {
        fail(`${path} must not remain tracked by Git`);
      }
    }
  };

  if (requireTrackedFiles) {
    const corePath = relative(root, fileURLToPath(import.meta.url)).replaceAll("\\", "/");
    requireTracked(corePath);
  }

  const readText = (path) => {
    if (requireTrackedFiles) {
      requireTracked(path);
    }
    return readFileSync(assertRegularFile(path), "utf8");
  };

  const readJson = (path) => {
    try {
      return JSON.parse(readText(path));
    } catch {
      fail(`${path} is not valid JSON`);
    }
  };

  const fingerprintFile = (path) =>
    createHash("sha256").update(readFileSync(assertRegularFile(path))).digest("hex");

  const listFiles = (path) => {
    resolveRepositoryPath(path);
    const directory = assertRepositoryDirectory(path);
    const prefix = `${path}/`;
    if (requireTrackedFiles) {
      const files = [...loadTrackedFiles()].filter((file) => file.startsWith(prefix));
      if (files.length === 0) {
        fail(`${path} has no tracked files`);
      }
      return files;
    }

    const files = [];
    const visit = (current) => {
      for (const entry of readdirSync(current).sort()) {
        const absolute = resolve(current, entry);
        const status = lstatSync(absolute);
        if (status.isSymbolicLink()) {
          fail(`${relative(root, absolute)} must not be a symbolic link`);
        }
        if (status.isDirectory()) {
          visit(absolute);
        } else if (status.isFile()) {
          files.push(relative(root, absolute));
        } else {
          fail(`${relative(root, absolute)} is not a regular file`);
        }
      }
    };
    visit(directory);
    return files;
  };

  const loadUntrackedFiles = () => {
    const result = spawnSync(
      "git",
      ["ls-files", "--others", "--exclude-standard", "-z"],
      {
        cwd: root,
        encoding: "utf8",
        stdio: "pipe",
      },
    );
    if (result.error) {
      fail(`git ls-files --others --exclude-standard -z failed to start: ${result.error.message}`);
    }
    if (result.status !== 0) {
      process.stdout.write(result.stdout ?? "");
      process.stderr.write(result.stderr ?? "");
      process.exit(result.status ?? 1);
    }
    return result.stdout.split("\0").filter(Boolean);
  };

  const assertContains = (path, needle) => {
    if (!readText(path).includes(needle)) {
      fail(`${path} does not contain ${needle}`);
    }
  };

  const assertNotContains = (path, needle) => {
    if (readText(path).includes(needle)) {
      fail(`${path} must not contain ${needle}`);
    }
  };

  const assertMinOccurrences = (path, needle, expectedMin) => {
    const count = readText(path).split(needle).length - 1;
    if (count < expectedMin) {
      fail(`${path} contains ${needle} ${count} time(s), expected at least ${expectedMin}`);
    }
  };

  const assertTextPolicy = (policy) => {
    const files = policy?.files ?? [];
    if (!Array.isArray(files) || files.length === 0) {
      fail("text policy requires at least one file policy");
    }

    for (const filePolicy of files) {
      const {
        path,
        required = [],
        forbidden = [],
        minimumOccurrences = [],
        requiredMatches = [],
        forbiddenMatches = [],
      } = filePolicy ?? {};
      if (typeof path !== "string" || path.length === 0) {
        fail("text file policy requires a path");
      }
      for (const [policyName, needles] of [
        ["required", required],
        ["forbidden", forbidden],
      ]) {
        if (
          !Array.isArray(needles) ||
          needles.some((needle) => typeof needle !== "string")
        ) {
          fail(`${path} ${policyName} text policy must be an array of strings`);
        }
      }
      if (!Array.isArray(minimumOccurrences)) {
        fail(`${path} minimum-occurrence policy must be an array`);
      }
      if (!Array.isArray(requiredMatches) || !Array.isArray(forbiddenMatches)) {
        fail(`${path} regular-expression policies must be arrays`);
      }
      for (const needle of required) {
        assertContains(path, needle);
      }
      for (const needle of forbidden) {
        assertNotContains(path, needle);
      }
      for (const occurrence of minimumOccurrences) {
        const { needle, count } = occurrence ?? {};
        if (typeof needle !== "string" || !Number.isSafeInteger(count) || count < 0) {
          fail(`${path} has an invalid minimum-occurrence policy`);
        }
        assertMinOccurrences(path, needle, count);
      }
      for (const matchPolicy of requiredMatches) {
        const { pattern, description } = matchPolicy ?? {};
        requireMatch(path, pattern, description);
      }
      for (const matchPolicy of forbiddenMatches) {
        const { pattern, description } = matchPolicy ?? {};
        assertNotMatches(path, pattern, description);
      }
    }
  };

  const clonePattern = (pattern) => {
    if (!(pattern instanceof RegExp)) {
      fail("release readiness match assertions require a RegExp");
    }
    return new RegExp(pattern.source, pattern.flags);
  };

  const requireMatch = (path, pattern, description) => {
    // Clone caller-provided expressions so global or sticky regex state cannot
    // make a repeated release check depend on an earlier invocation.
    const match = clonePattern(pattern).exec(readText(path));
    if (match === null) {
      fail(`${path} does not contain ${description}`);
    }
    return match;
  };

  const assertNotMatches = (path, pattern, description) => {
    // Keep this assertion deterministic when a shared expression uses `g` or `y`.
    if (clonePattern(pattern).test(readText(path))) {
      fail(`${path} must not contain ${description}`);
    }
  };

  const assertLockPackageVersion = (lock, name, version, source = null) => {
    const blocks = lock.match(/\[\[package\]\]\n[\s\S]*?(?=\n\[\[package\]\]|\n*$)/g) ?? [];
    const block = blocks.find(
      (candidate) =>
        candidate.includes(`name = "${name}"\n`) && candidate.includes(`version = "${version}"\n`),
    );
    if (block === undefined) {
      fail(`Cargo.lock does not pin ${name} ${version}`);
    }
    if (source !== null && !block.includes(`source = "${source}"\n`)) {
      fail(`Cargo.lock ${name} ${version} does not use ${source}`);
    }
  };

  const runNodeCheck = (scriptPath, args = []) => {
    const result = spawnSync(process.execPath, [scriptPath, ...args], {
      cwd: root,
      encoding: "utf8",
      stdio: "pipe",
    });
    if (result.error) {
      fail(`${[scriptPath, ...args].join(" ")} failed to start: ${result.error.message}`);
    }
    if (result.status !== 0) {
      const output = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
      const command = [scriptPath, ...args].join(" ");
      fail(`${command} failed${output.length === 0 ? "" : `:\n${output}`}`);
    }
  };

  const packageList = (packageName) => {
    const args = ["package", "--list", "-p", packageName];
    if (process.env.GITHUB_ACTIONS !== "true") {
      args.push("--allow-dirty");
    }
    return run("cargo", args, { capture: true }).stdout.split(/\r?\n/u).filter(Boolean);
  };

  const assertPackageFiles = (packageName, requiredFiles) => {
    const files = new Set(packageList(packageName));
    for (const file of requiredFiles) {
      if (!files.has(file)) {
        fail(`${packageName} package is missing ${file}`);
      }
    }
  };

  const runCommands = (commands) => {
    if (!Array.isArray(commands) || commands.length === 0) {
      fail("command policy requires at least one command");
    }
    for (const entry of commands) {
      if (!Array.isArray(entry) || entry.length < 2 || entry.length > 3) {
        fail("command policy entries must be [command, args, options?] tuples");
      }
      const [command, args, options] = entry;
      if (
        options !== undefined &&
        (options === null || typeof options !== "object" || Array.isArray(options))
      ) {
        fail("command policy options must be an object");
      }
      run(command, args, options ?? {});
    }
  };

  const validateLatestStableVersions = (versions = {}) => {
    if (versions === null || typeof versions !== "object" || Array.isArray(versions)) {
      fail("latest stable version overrides must be an explicit mapping");
    }
    const parsed = new Map();
    for (const [crateName, version] of Object.entries(versions)) {
      if (!/^[a-z][a-z0-9_-]*$/u.test(crateName)) {
        fail("latest stable version override names must be crate identifiers");
      }
      if (typeof version !== "string" || !/^\d+\.\d+\.\d+$/u.test(version)) {
        fail(`${crateName} latest stable override must be an exact stable version`);
      }
      parsed.set(crateName, version);
    }
    return parsed;
  };

  const loadLatestCargoRegistryVersion = (crateName, latestStableVersions) => {
    if (!/^[a-z][a-z0-9_-]*$/u.test(crateName)) {
      fail("latest Cargo registry version lookup requires a crate identifier");
    }
    const overriddenVersion = latestStableVersions.get(crateName);
    if (overriddenVersion !== undefined) {
      return overriddenVersion;
    }
    const result = run("cargo", ["search", crateName, "--limit", "20"], {
      capture: true,
    });
    const match = new RegExp(
      `^${escapeRegExpLiteral(crateName)}\\s*=\\s*"([^"]+)"`,
      "mu",
    ).exec(result.stdout);
    if (match === null) {
      fail(`cargo search did not return ${crateName}`);
    }
    if (!/^\d+\.\d+\.\d+$/u.test(match[1])) {
      fail(`${crateName} latest Cargo registry version is not stable`);
    }
    return match[1];
  };

  const expectedLatestCargoRequirement = (
    crateName,
    requirementStyle,
    latestStableVersions,
  ) => {
    if (!["caret", "exact"].includes(requirementStyle)) {
      fail(`${crateName} latest stable requirement policy is invalid`);
    }
    const version = loadLatestCargoRegistryVersion(crateName, latestStableVersions);
    return requirementStyle === "caret" ? `^${version}` : version;
  };

  const normalizeLatestStableDependencyPolicy = (policy) => {
    if (policy === undefined || policy === false) {
      return null;
    }
    if (policy === true) {
      return {
        names: DEFAULT_REALLYME_LATEST_STABLE_DEPENDENCIES,
        requirement: "caret",
      };
    }
    if (
      policy === null ||
      typeof policy !== "object" ||
      Array.isArray(policy) ||
      !Object.keys(policy).every((key) => ["names", "requirement"].includes(key))
    ) {
      fail("latest stable dependency policy must be true or an explicit mapping");
    }
    const names = policy.names ?? DEFAULT_REALLYME_LATEST_STABLE_DEPENDENCIES;
    const requirement = policy.requirement ?? "caret";
    if (
      !Array.isArray(names) ||
      names.length === 0 ||
      names.some((name) => typeof name !== "string" || !/^[a-z][a-z0-9_-]*$/u.test(name))
    ) {
      fail("latest stable dependency policy names must be crate identifiers");
    }
    if (!["caret", "exact"].includes(requirement)) {
      fail("latest stable dependency policy requirement must be caret or exact");
    }
    return { names, requirement };
  };

  const assertCargoMetadataDocument = (metadata, policy) => {
    const packages = policy?.packages ?? [];
    const latestStableVersions = validateLatestStableVersions(policy?.latestStableVersions);
    const latestStableDependencyPolicy = normalizeLatestStableDependencyPolicy(
      policy?.reallyMeLatestStableDependencies,
    );
    if (
      metadata === null ||
      typeof metadata !== "object" ||
      !Array.isArray(metadata.packages)
    ) {
      fail("cargo metadata did not return a packages array");
    }
    if (
      !Array.isArray(packages) ||
      (packages.length === 0 && latestStableDependencyPolicy === null)
    ) {
      fail("cargo metadata policy requires at least one package");
    }

    const packagesByName = new Map();
    for (const cargoPackage of metadata.packages) {
      if (
        cargoPackage !== null &&
        typeof cargoPackage === "object" &&
        typeof cargoPackage.name === "string"
      ) {
        if (packagesByName.has(cargoPackage.name)) {
          fail(`cargo metadata contains duplicate package ${cargoPackage.name}`);
        }
        packagesByName.set(cargoPackage.name, cargoPackage);
      }
    }

    for (const packagePolicy of packages) {
      const {
        name,
        version,
        publish = "any",
        dependencies = [],
        packageFiles = [],
      } = packagePolicy ?? {};
      if (typeof name !== "string" || name.length === 0) {
        fail("cargo metadata package policy requires a name");
      }
      const cargoPackage = packagesByName.get(name);
      if (cargoPackage === undefined) {
        fail(`cargo metadata did not expose ${name}`);
      }
      if (version !== undefined && cargoPackage.version !== version) {
        fail(`${name} metadata version is ${cargoPackage.version}, expected ${version}`);
      }
      const isPublishable =
        cargoPackage.publish === null ||
        (Array.isArray(cargoPackage.publish) && cargoPackage.publish.length > 0);
      if (publish === "public" && !isPublishable) {
        fail(`${name} must be publishable`);
      }
      if (publish === "private" && isPublishable) {
        fail(`${name} must set publish = false`);
      }
      if (!["any", "public", "private"].includes(publish)) {
        fail(`${name} has an invalid publish policy`);
      }
      if (!Array.isArray(cargoPackage.dependencies)) {
        fail(`${name} metadata dependencies are malformed`);
      }
      if (!Array.isArray(dependencies)) {
        fail(`${name} dependency policy must be an array`);
      }
      if (
        !Array.isArray(packageFiles) ||
        packageFiles.some((file) => typeof file !== "string" || file.length === 0)
      ) {
        fail(`${name} package file policy must be an array of non-empty strings`);
      }

      for (const dependencyPolicy of dependencies) {
        const {
          name: dependencyName,
          requirement,
          source = "any",
          defaultFeatures,
          optional,
          features,
          kind,
          target,
          rename,
          latestStable = false,
        } = dependencyPolicy ?? {};
        if (typeof dependencyName !== "string" || dependencyName.length === 0) {
          fail(`${name} dependency policy requires a name`);
        }
        if (
          !(
            latestStable === false ||
            latestStable === true ||
            latestStable === "caret" ||
            latestStable === "exact"
          )
        ) {
          fail(`${name} dependency ${dependencyName} latest stable policy is invalid`);
        }
        if (requirement !== undefined && latestStable !== false) {
          fail(
            `${name} dependency ${dependencyName} must configure either an exact requirement or latest stable policy`,
          );
        }
        const candidates = cargoPackage.dependencies.filter(
          (candidate) =>
            candidate.name === dependencyName &&
            (kind === undefined || candidate.kind === kind) &&
            (target === undefined || candidate.target === target) &&
            (rename === undefined || candidate.rename === rename),
        );
        if (candidates.length === 0) {
          fail(`${name} is missing ${dependencyName}`);
        }
        if (candidates.length > 1) {
          fail(
            `${name} dependency ${dependencyName} is ambiguous; specify kind, target, or rename`,
          );
        }
        const [dependency] = candidates;
        const expectedRequirement =
          latestStable === false
            ? requirement
            : expectedLatestCargoRequirement(
                dependencyName,
                latestStable === true ? "caret" : latestStable,
                latestStableVersions,
              );
        if (expectedRequirement !== undefined && dependency.req !== expectedRequirement) {
          fail(
            `${name} dependency ${dependencyName} requirement is ${dependency.req}, expected ${expectedRequirement}`,
          );
        }
        if (
          source === "registry" &&
          (typeof dependency.source !== "string" ||
            !dependency.source.startsWith("registry+"))
        ) {
          fail(`${name} dependency ${dependencyName} must resolve from a registry`);
        }
        if (source === "path" && dependency.source !== null) {
          fail(`${name} dependency ${dependencyName} must resolve from a path`);
        }
        if (!["any", "registry", "path"].includes(source)) {
          fail(`${name} dependency ${dependencyName} has an invalid source policy`);
        }
        if (
          defaultFeatures !== undefined &&
          dependency.uses_default_features !== defaultFeatures
        ) {
          fail(
            `${name} dependency ${dependencyName} default-features policy does not match`,
          );
        }
        if (optional !== undefined && dependency.optional !== optional) {
          fail(`${name} dependency ${dependencyName} optional policy does not match`);
        }
        if (features !== undefined) {
          if (!Array.isArray(features) || features.some((feature) => typeof feature !== "string")) {
            fail(`${name} dependency ${dependencyName} features policy is invalid`);
          }
          const actualFeatures = new Set(dependency.features ?? []);
          for (const feature of features) {
            if (!actualFeatures.has(feature)) {
              fail(`${name} dependency ${dependencyName} is missing feature ${feature}`);
            }
          }
        }
      }

      if (packageFiles.length > 0) {
        assertPackageFiles(name, packageFiles);
      }
    }

    if (latestStableDependencyPolicy !== null) {
      const dependencyNames = new Set(latestStableDependencyPolicy.names);
      for (const cargoPackage of metadata.packages) {
        if (
          cargoPackage === null ||
          typeof cargoPackage !== "object" ||
          typeof cargoPackage.name !== "string" ||
          !Array.isArray(cargoPackage.dependencies)
        ) {
          continue;
        }
        for (const dependency of cargoPackage.dependencies) {
          if (
            dependency === null ||
            typeof dependency !== "object" ||
            typeof dependency.name !== "string" ||
            !dependencyNames.has(dependency.name)
          ) {
            continue;
          }
          if (
            typeof dependency.source !== "string" ||
            !dependency.source.startsWith("registry+")
          ) {
            fail(
              `${cargoPackage.name} dependency ${dependency.name} must use a registry source for latest stable enforcement`,
            );
          }
          const expectedRequirement = expectedLatestCargoRequirement(
            dependency.name,
            latestStableDependencyPolicy.requirement,
            latestStableVersions,
          );
          if (dependency.req !== expectedRequirement) {
            fail(
              `${cargoPackage.name} dependency ${dependency.name} requirement is ${dependency.req}, expected latest stable ${expectedRequirement}`,
            );
          }
        }
      }
    }

    return packagesByName;
  };

  const assertCargoMetadataPolicy = (policy) => {
    const metadataArgs = policy?.metadataArgs ?? [
      "metadata",
      "--format-version",
      "1",
      "--no-deps",
    ];
    const metadataResult = run("cargo", metadataArgs, { capture: true });
    let metadata;
    try {
      metadata = JSON.parse(metadataResult.stdout);
    } catch {
      fail("cargo metadata returned malformed JSON");
    }
    return assertCargoMetadataDocument(metadata, policy);
  };

  const assertCargoWorkspacePolicy = (policy = {}) => {
    const {
      requireWorkspaceLints = true,
      requirePublishInclude = true,
      validatePublishablePathDependencies = true,
    } = policy;
    const metadataResult = run(
      "cargo",
      ["metadata", "--format-version", "1", "--no-deps"],
      { capture: true },
    );
    let metadata;
    try {
      metadata = JSON.parse(metadataResult.stdout);
    } catch {
      fail("cargo metadata returned malformed JSON");
    }
    if (
      !Array.isArray(metadata.packages) ||
      !Array.isArray(metadata.workspace_members)
    ) {
      fail("cargo workspace metadata is malformed");
    }

    const workspaceIds = new Set(metadata.workspace_members);
    const workspacePackages = metadata.packages.filter((cargoPackage) =>
      workspaceIds.has(cargoPackage.id),
    );
    const publishableByName = new Map();
    const parseSemver = (version) => {
      const match = /^(\d+)\.(\d+)\.(\d+)$/u.exec(version);
      if (match === null) {
        return null;
      }
      return match.slice(1).map((part) => Number.parseInt(part, 10));
    };
    const caretIncludes = (requirement, version) => {
      if (!requirement.startsWith("^")) {
        return requirement === version || requirement === `=${version}`;
      }
      const minimum = parseSemver(requirement.slice(1));
      const actual = parseSemver(version);
      if (minimum === null || actual === null || actual[0] !== minimum[0]) {
        return false;
      }
      if (minimum[0] === 0 && actual[1] !== minimum[1]) {
        return false;
      }
      return (
        actual[1] > minimum[1] ||
        (actual[1] === minimum[1] && actual[2] >= minimum[2])
      );
    };
    for (const cargoPackage of workspacePackages) {
      const manifestPath = relative(root, cargoPackage.manifest_path).replaceAll("\\", "/");
      const manifest = readText(manifestPath);
      if (
        requireWorkspaceLints &&
        !/\[lints\]\s+workspace\s*=\s*true\b/u.test(manifest)
      ) {
        fail(`${manifestPath} must inherit workspace lints`);
      }
      const publishable =
        cargoPackage.publish === null ||
        (Array.isArray(cargoPackage.publish) && cargoPackage.publish.length > 0);
      if (publishable) {
        publishableByName.set(cargoPackage.name, cargoPackage);
        if (
          requirePublishInclude &&
          !/^include\s*=\s*\[/mu.test(manifest)
        ) {
          fail(`${manifestPath} publishable package must use an include allowlist`);
        }
      }
    }

    if (validatePublishablePathDependencies) {
      for (const cargoPackage of publishableByName.values()) {
        for (const dependency of cargoPackage.dependencies ?? []) {
          if (dependency.source !== null || typeof dependency.path !== "string") {
            continue;
          }
          const dependencyName = dependency.name;
          const target = publishableByName.get(dependencyName);
          if (target === undefined) {
            continue;
          }
          if (!caretIncludes(dependency.req, target.version)) {
            fail(
              `${cargoPackage.name} publishable path dependency ${dependencyName} ${dependency.req} does not match ${target.version}`,
            );
          }
        }
      }
    }
  };

  const assertSpdxHeaders = (policy = {}) => {
    const {
      extensions = [
        ".cjs",
        ".cts",
        ".js",
        ".jsx",
        ".kt",
        ".kts",
        ".mjs",
        ".mts",
        ".proto",
        ".py",
        ".rs",
        ".sh",
        ".swift",
        ".toml",
        ".ts",
        ".tsx",
        ".yaml",
        ".yml",
      ],
      names = [".gitignore"],
      excludedPrefixes = [],
      exclusions = [],
      requireExclusionsMatched = false,
      requireExclusionReasons = false,
      copyright = "SPDX-FileCopyrightText: 2026 ReallyMe LLC",
      license = "SPDX-License-Identifier: MIT OR Apache-2.0",
    } = policy;
    for (const [policyName, values] of [
      ["extensions", extensions],
      ["names", names],
      ["excluded prefixes", excludedPrefixes],
    ]) {
      if (!Array.isArray(values) || values.some((value) => typeof value !== "string")) {
        fail(`SPDX ${policyName} policy must be an array of strings`);
      }
    }
    if (!Array.isArray(exclusions)) {
      fail("SPDX exclusions policy must be an array");
    }
    if (typeof requireExclusionsMatched !== "boolean") {
      fail("SPDX requireExclusionsMatched policy must be a boolean");
    }
    if (typeof requireExclusionReasons !== "boolean") {
      fail("SPDX requireExclusionReasons policy must be a boolean");
    }
    if (typeof copyright !== "string" || copyright.length === 0) {
      fail("SPDX copyright policy must be a non-empty string");
    }
    if (typeof license !== "string" || license.length === 0) {
      fail("SPDX license policy must be a non-empty string");
    }
    const allowedReasons = new Set(["generated", "third-party", "vendored"]);
    const normalizedExclusions = excludedPrefixes.map((path) => ({ path, reason: null }));
    for (const [index, exclusion] of exclusions.entries()) {
      if (
        exclusion === null ||
        typeof exclusion !== "object" ||
        Array.isArray(exclusion) ||
        typeof exclusion.path !== "string" ||
        exclusion.path.length === 0 ||
        typeof exclusion.reason !== "string" ||
        !allowedReasons.has(exclusion.reason)
      ) {
        fail(
          `SPDX exclusions[${index}] must define a path and a generated, third-party, or vendored reason`,
        );
      }
      normalizedExclusions.push({ path: exclusion.path, reason: exclusion.reason });
    }
    const exclusionPaths = new Set();
    for (const exclusion of normalizedExclusions) {
      const absolute = resolveRepositoryPath(exclusion.path, "SPDX exclusion path");
      const path = relative(root, absolute).replaceAll("\\", "/").replace(/\/+$/u, "");
      if (path.length === 0) {
        fail("SPDX exclusion path must not be the repository root");
      }
      if (exclusionPaths.has(path)) {
        fail(`SPDX exclusion path ${path} is duplicated`);
      }
      if (requireExclusionReasons && exclusion.reason === null) {
        fail(`SPDX exclusion path ${path} requires a typed reason`);
      }
      exclusionPaths.add(path);
    }
    const extensionSet = new Set(extensions);
    const nameSet = new Set(names);
    const trackedFiles = [...loadTrackedFiles()];
    const requiresSpdxHeader = (path) => {
      const fileName = path.slice(path.lastIndexOf("/") + 1);
      return nameSet.has(fileName) || extensionSet.has(extname(fileName));
    };
    if (requireExclusionsMatched) {
      for (const path of exclusionPaths) {
        if (
          !trackedFiles.some(
            (trackedPath) => pathIsInside(trackedPath, path) && requiresSpdxHeader(trackedPath),
          )
        ) {
          fail(`SPDX exclusion path ${path} does not match a governed tracked file`);
        }
      }
    }
    for (const path of trackedFiles) {
      if ([...exclusionPaths].some((prefix) => pathIsInside(path, prefix))) {
        continue;
      }
      if (!requiresSpdxHeader(path)) {
        continue;
      }
      const text = readText(path);
      if (!text.includes(copyright)) {
        fail(`${path} is missing the configured SPDX copyright header`);
      }
      if (!text.includes(license)) {
        fail(`${path} is missing the configured SPDX license header`);
      }
    }
  };

  const assertRepositoryShapePolicy = (policy) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("repository shape policy must be an object");
    }
    const archetypes = {
      "foundational-library": {
        required: ["crates", "docs", "scripts", ".github"],
        permitted: [
          "crates",
          "bindings",
          "gen",
          "packages",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
        ],
      },
      "protocol-engine": {
        required: ["crates", "contracts", "conformance", "docs", "scripts", ".github"],
        permitted: [
          "crates",
          "bindings",
          "gen",
          "packages",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
        ],
      },
      "developer-platform": {
        required: [
          "bindings",
          "gen",
          "packages",
          "examples",
          "contracts",
          "conformance",
          "docs",
          "scripts",
          ".github",
        ],
        permitted: [
          "crates",
          "bindings",
          "gen",
          "packages",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
          ".changeset",
          "gradle",
        ],
      },
      "application": {
        required: ["contracts", "conformance", "docs", "scripts", ".github"],
        requiredAny: [["app", "crates"]],
        permitted: [
          "app",
          "crates",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
          "config",
          "migrations",
          "resources",
        ],
      },
      "application-collection": {
        required: ["apps", "conformance", "docs", "scripts", ".github"],
        permitted: [
          "apps",
          "crates",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
        ],
      },
      "product-workspace": {
        required: ["apps", "crates", "conformance", "docs", "scripts", ".github"],
        permitted: [
          "apps",
          "crates",
          "bindings",
          "gen",
          "packages",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
          "deploy",
        ],
      },
      "hosted-service": {
        required: [
          "services",
          "deploy",
          "operations",
          "conformance",
          "docs",
          "scripts",
          ".github",
        ],
        permitted: [
          "crates",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
          "services",
          "deploy",
          "migrations",
          "operations",
          "config",
          "docker",
        ],
      },
      "platform-workspace": {
        required: [
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
        permitted: [
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
      },
      "runtime-composition": {
        required: [
          "crates",
          "configs",
          "deploy",
          "contracts",
          "conformance",
          "docs",
          "scripts",
          ".github",
        ],
        permitted: [
          "crates",
          "configs",
          "deploy",
          "contracts",
          "conformance",
          "vectors",
          "fuzz",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
          "docker",
        ],
      },
      "infrastructure": {
        required: [
          "deployments",
          "operations",
          "docs",
          "scripts",
          ".github",
        ],
        permitted: [
          "topology",
          "provisioning",
          "configuration",
          "deployments",
          "networking",
          "observability",
          "operations",
          "crates",
          "tools",
          "contracts",
          "conformance",
          "vectors",
          "examples",
          "docs",
          "scripts",
          ".github",
          ".cargo",
        ],
      },
      "conformance-suite": {
        required: [
          "upstream",
          "plans",
          "adapters",
          "schemas",
          "tests",
          "results",
          "evidence",
          "docs",
          "scripts",
          ".github",
        ],
        permitted: [
          "contracts",
          "vectors",
          "examples",
          "docs",
          "scripts",
          ".github",
          "upstream",
          "plans",
          "adapters",
          "schemas",
          "tests",
          "results",
          "evidence",
        ],
      },
      "taxonomy": {
        required: [
          "taxonomy",
          "schema",
          "views",
          "consumers",
          "conformance",
          "tests",
          "docs",
          "scripts",
          ".github",
        ],
        permitted: [
          "taxonomy",
          "schema",
          "views",
          "consumers",
          "conformance",
          "tests",
          "docs",
          "scripts",
          ".github",
        ],
      },
      "documentation-site": {
        required: ["content", "contracts", "tests", "scripts", ".github"],
        permitted: [
          "content",
          "components",
          "snippets",
          "contracts",
          "conformance",
          "examples",
          "localization",
          "assets",
          "docs",
          "tests",
          "scripts",
          ".github",
        ],
      },
      "tooling": {
        required: ["test", "docs", "scripts", ".github"],
        permitted: [
          "contracts",
          "conformance",
          "vectors",
          "examples",
          "docs",
          "scripts",
          ".github",
          "templates",
          "test",
          "fixtures",
        ],
      },
    };
    const {
      archetype,
      requiredLanes = [],
      optionalLanes = [],
      exceptions = [],
      crates = [],
      subLanes = {},
      forbiddenPaths = [],
      requireReleaseReadiness = archetype !== "tooling",
    } = policy;
    const archetypePolicy = archetypes[archetype];
    if (archetypePolicy === undefined) {
      fail(`repository shape archetype ${String(archetype)} is not approved`);
    }
    for (const [name, values] of [
      ["requiredLanes", requiredLanes],
      ["optionalLanes", optionalLanes],
      ["forbiddenPaths", forbiddenPaths],
    ]) {
      if (
        !Array.isArray(values) ||
        values.some((value) => typeof value !== "string" || value.length === 0)
      ) {
        fail(`repository shape ${name} must be an array of non-empty strings`);
      }
      if (new Set(values).size !== values.length) {
        fail(`repository shape ${name} must not contain duplicates`);
      }
    }
    if (typeof requireReleaseReadiness !== "boolean") {
      fail("repository shape requireReleaseReadiness must be a boolean");
    }
    if (!requireReleaseReadiness && archetype !== "tooling") {
      fail("repository shape permits disabling release readiness only for tooling");
    }
    const requiredLaneSet = new Set(requiredLanes);
    const optionalLaneSet = new Set(optionalLanes);
    for (const lane of requiredLaneSet) {
      if (optionalLaneSet.has(lane)) {
        fail(`repository shape lane ${lane} cannot be both required and optional`);
      }
    }
    const permittedLaneSet = new Set(archetypePolicy.permitted);
    for (const lane of [...requiredLaneSet, ...optionalLaneSet]) {
      if (!permittedLaneSet.has(lane)) {
        fail(`repository shape archetype ${archetype} does not permit lane ${lane}`);
      }
    }
    for (const lane of archetypePolicy.required) {
      if (!requiredLaneSet.has(lane)) {
        fail(`repository shape archetype ${archetype} requires lane ${lane}`);
      }
    }
    for (const alternatives of archetypePolicy.requiredAny ?? []) {
      if (!alternatives.some((lane) => requiredLaneSet.has(lane))) {
        fail(
          `repository shape archetype ${archetype} requires at least one implementation lane from ${alternatives.join(
            ", ",
          )}`,
        );
      }
    }

    if (!Array.isArray(exceptions)) {
      fail("repository shape exceptions must be an array");
    }
    const exceptionReasons = new Set([
      "build-tool",
      "deployment",
      "generated",
      "organization-specific",
      "third-party",
      "vendored",
    ]);
    const exceptionPaths = new Set();
    for (const [index, entry] of exceptions.entries()) {
      if (
        entry === null ||
        typeof entry !== "object" ||
        Array.isArray(entry) ||
        typeof entry.path !== "string" ||
        !/^[.]?[A-Za-z0-9][A-Za-z0-9_.-]*$/u.test(entry.path) ||
        typeof entry.reason !== "string" ||
        !exceptionReasons.has(entry.reason)
      ) {
        fail(`repository shape exceptions[${index}] must define a root lane and typed reason`);
      }
      if (exceptionPaths.has(entry.path)) {
        fail(`repository shape exception ${entry.path} is duplicated`);
      }
      if (permittedLaneSet.has(entry.path)) {
        fail(`repository shape exception ${entry.path} is unnecessary for this archetype`);
      }
      exceptionPaths.add(entry.path);
    }

    const repositoryFiles = new Set(loadTrackedFiles());
    if (!requireTrackedFiles) {
      for (const path of loadUntrackedFiles()) {
        repositoryFiles.add(path);
      }
    }
    const governedFiles = [...repositoryFiles].map((path) => path.replaceAll("\\", "/"));
    const observedRootLanes = new Set(
      governedFiles
        .filter((path) => path.includes("/"))
        .map((path) => path.slice(0, path.indexOf("/"))),
    );
    const archetypesWithRootTests = new Set([
      "conformance-suite",
      "taxonomy",
      "documentation-site",
    ]);
    const forbiddenRootLanes = new Set([
      "src",
      "proto",
      "protos",
      "generated",
      ...(archetypesWithRootTests.has(archetype) ? [] : ["tests"]),
    ]);
    for (const lane of observedRootLanes) {
      if (forbiddenRootLanes.has(lane)) {
        fail(`repository shape forbids root lane ${lane}`);
      }
      if (
        !requiredLaneSet.has(lane) &&
        !optionalLaneSet.has(lane) &&
        !exceptionPaths.has(lane)
      ) {
        fail(`repository shape has undeclared root lane ${lane}`);
      }
    }
    for (const lane of requiredLaneSet) {
      if (!observedRootLanes.has(lane)) {
        fail(`repository shape required lane ${lane} has no tracked files`);
      }
    }
    for (const path of exceptionPaths) {
      if (!observedRootLanes.has(path)) {
        fail(`repository shape exception ${path} does not match a tracked root lane`);
      }
    }
    assertPathsAbsent([...forbiddenRootLanes, ...forbiddenPaths]);
    if (governedFiles.some((path) => pathIsInside(path, "conformance/vectors"))) {
      fail("repository shape requires reusable vectors at root vectors/, not conformance/vectors/");
    }

    if (subLanes === null || typeof subLanes !== "object" || Array.isArray(subLanes)) {
      fail("repository shape subLanes must be an object");
    }
    const subLaneParents = new Set([
      "bindings",
      "gen",
      "packages",
      ...(archetype === "hosted-service" ? ["services"] : []),
      ...(archetype === "taxonomy" ? ["views"] : []),
      ...(archetype === "documentation-site" ? ["content"] : []),
      ...(archetype === "product-workspace" ? ["apps"] : []),
      ...(archetype === "platform-workspace"
        ? ["apps", "kits", "servers", "workers"]
        : archetype === "application-collection"
          ? ["apps"]
        : []),
    ]);
    for (const [parent, children] of Object.entries(subLanes)) {
      if (!subLaneParents.has(parent)) {
        fail(`repository shape does not support sublane declarations for ${parent}`);
      }
      if (
        !Array.isArray(children) ||
        children.length === 0 ||
        children.some(
          (child) =>
            typeof child !== "string" ||
            !/^[A-Za-z0-9][A-Za-z0-9_.-]*$/u.test(child),
        ) ||
        new Set(children).size !== children.length
      ) {
        fail(`repository shape ${parent} sublanes must be a non-empty array of unique names`);
      }
      if (
        parent === "apps" &&
        (archetype === "application-collection" || archetype === "product-workspace") &&
        children.length < 2
      ) {
        fail(`repository shape ${archetype} requires at least two application sublanes`);
      }
      if (!observedRootLanes.has(parent)) {
        fail(`repository shape ${parent} sublanes are configured for an absent lane`);
      }
      const observedChildren = new Set(
        governedFiles
          .filter((path) => path.startsWith(`${parent}/`) && path.slice(parent.length + 1).includes("/"))
          .map((path) => path.slice(parent.length + 1).split("/", 1)[0]),
      );
      for (const child of observedChildren) {
        if (!children.includes(child)) {
          fail(`repository shape has undeclared ${parent} sublane ${child}`);
        }
      }
      for (const child of children) {
        if (!observedChildren.has(child)) {
          fail(`repository shape ${parent} sublane ${child} has no tracked files`);
        }
      }
    }
    for (const parent of subLaneParents) {
      if (observedRootLanes.has(parent) && !Object.hasOwn(subLanes, parent)) {
        fail(`repository shape lane ${parent} requires explicit sublane declarations`);
      }
    }

    if (!Array.isArray(crates)) {
      fail("repository shape crates must be an array");
    }
    const crateRoles = new Set([
      "adapter",
      "domain",
      "facade",
      "proto",
      "proto-codec",
      "provider",
      "runtime",
      "storage",
      "support",
      "test-support",
      "transport",
    ]);
    const declaredCrates = new Map();
    for (const [index, entry] of crates.entries()) {
      if (
        entry === null ||
        typeof entry !== "object" ||
        Array.isArray(entry) ||
        typeof entry.path !== "string" ||
        !/^crates\/[A-Za-z0-9][A-Za-z0-9_.-]*(?:\/[A-Za-z0-9][A-Za-z0-9_.-]*)*$/u.test(
          entry.path,
        ) ||
        typeof entry.role !== "string" ||
        !crateRoles.has(entry.role)
      ) {
        fail(`repository shape crates[${index}] must define a crates/ path and approved role`);
      }
      resolveRepositoryPath(entry.path, `repository shape crate ${entry.path}`);
      if (declaredCrates.has(entry.path)) {
        fail(`repository shape crate ${entry.path} is duplicated`);
      }
      requireTracked(`${entry.path}/Cargo.toml`);
      declaredCrates.set(entry.path, entry.role);
    }
    const observedCrates = governedFiles
      .filter((path) => path.startsWith("crates/") && path.endsWith("/Cargo.toml"))
      .map((path) => path.slice(0, -"/Cargo.toml".length));
    for (const path of observedCrates) {
      if (path.startsWith("crates/proto-") && path !== "crates/proto-codec") {
        fail(
          `repository shape forbids compatibility proto package ${path}; generated protobuf modules belong in crates/proto`,
        );
      }
    }
    for (const path of observedCrates) {
      if (!declaredCrates.has(path)) {
        fail(`repository shape Cargo crate ${path} is undeclared`);
      }
    }
    if (observedRootLanes.has("crates")) {
      requireTracked("Cargo.toml");
      const rootCargo = readText("Cargo.toml");
      if (!/^\[workspace\][ \t]*$/mu.test(rootCargo)) {
        fail("repository shape with crates/ requires a root Cargo workspace");
      }
      if (/^\[package\][ \t]*$/mu.test(rootCargo)) {
        fail("repository shape root Cargo.toml must not define a root package");
      }
      if (declaredCrates.size === 0) {
        fail("repository shape crates/ lane requires declared Cargo crates");
      }
    } else if (crates.length !== 0) {
      fail("repository shape declares Cargo crates without a crates/ lane");
    }
    const protoCrates = [...declaredCrates].filter(([, role]) => role === "proto");
    const protoCodecCrates = [...declaredCrates].filter(([, role]) => role === "proto-codec");
    const facadeCrates = [...declaredCrates].filter(([, role]) => role === "facade");
    if (facadeCrates.length > 1) {
      fail("repository shape permits at most one facade crate");
    }
    if (
      facadeCrates.length === 1 &&
      !governedFiles.some(
        (path) =>
          path.endsWith("/Cargo.toml") && path !== `${facadeCrates[0][0]}/Cargo.toml`,
      )
    ) {
      fail("repository shape facade crate requires at least one internal package");
    }
    if (protoCrates.length > 1 || protoCodecCrates.length > 1) {
      fail("repository shape permits at most one proto and one proto-codec crate");
    }
    if (protoCrates.length === 1 && protoCrates[0][0] !== "crates/proto") {
      fail("repository shape canonical proto crate must be crates/proto");
    }
    if (protoCodecCrates.length === 1 && protoCodecCrates[0][0] !== "crates/proto-codec") {
      fail("repository shape proto-codec crate must be crates/proto-codec");
    }
    if (protoCodecCrates.length === 1 && protoCrates.length === 0) {
      fail("repository shape proto-codec requires a canonical proto crate");
    }
    const protoFiles = governedFiles.filter((path) => path.endsWith(".proto"));
    const appProtoPattern =
      /^apps\/[A-Za-z0-9][A-Za-z0-9_.-]*\/contract\/proto\/.+[.]proto$/u;
    const standaloneAppProtoPattern = /^app\/contract\/proto\/.+[.]proto$/u;
    const isCanonicalProto = (path) => pathIsInside(path, "crates/proto");
    const isApplicationProto = (path) => appProtoPattern.test(path);
    const isStandaloneApplicationProto = (path) =>
      standaloneAppProtoPattern.test(path);

    if (archetype === "platform-workspace") {
      if (protoCrates.length !== 0 || protoCodecCrates.length !== 0) {
        fail(
          "repository shape platform-workspace keeps protobuf ownership in application contracts",
        );
      }
      if (protoFiles.some((path) => !isApplicationProto(path))) {
        fail(
          "repository shape platform-workspace requires protobuf schemas in apps/<app>/contract/proto",
        );
      }
    } else if (archetype === "application") {
      if (
        protoFiles.some((path) => isCanonicalProto(path)) &&
        protoCrates.length === 0
      ) {
        fail(
          "repository shape application found shared Rust protobuf schemas without a declared canonical proto crate",
        );
      }
      if (
        protoFiles.some(
          (path) =>
            !isCanonicalProto(path) && !isStandaloneApplicationProto(path),
        )
      ) {
        fail(
          "repository shape application requires Rust/shared protobuf schemas in crates/proto and app-owned schemas in app/contract/proto",
        );
      }
    } else if (archetype === "application-collection") {
      if (
        protoFiles.some((path) => isCanonicalProto(path)) &&
        protoCrates.length === 0
      ) {
        fail(
          "repository shape application-collection found shared protobuf schemas without a declared canonical proto crate",
        );
      }
      if (
        protoFiles.some(
          (path) => !isCanonicalProto(path) && !isApplicationProto(path),
        )
      ) {
        fail(
          "repository shape application-collection requires shared protobuf schemas in crates/proto and application-owned schemas in apps/<app>/contract/proto",
        );
      }
    } else {
      if (protoFiles.length !== 0 && protoCrates.length === 0) {
        fail("repository shape found protobuf schemas without a declared canonical proto crate");
      }
      if (
        protoCrates.length === 1 &&
        protoFiles.some((path) => !isCanonicalProto(path))
      ) {
        fail("repository shape requires every protobuf schema inside crates/proto");
      }
    }
    if (
      governedFiles.some(
        (path) =>
          path.startsWith("crates/proto/") &&
          path.endsWith("/Cargo.toml") &&
          path !== "crates/proto/Cargo.toml",
      )
    ) {
      fail("repository shape forbids nested Cargo packages inside crates/proto");
    }

    if (requireReleaseReadiness) {
      requireTracked("scripts/check_release_readiness.mjs");
      requireTracked("scripts/release-readiness/core.mjs");
    }
  };

  const assertRustSourcePolicy = (policy = {}) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("Rust source policy must be an object");
    }
    const {
      roots = ["."],
      generatedPrefixes = [],
      baselinePath = null,
      productionTargetLines = 500,
      productionHardLines = 500,
      testTargetLines = 800,
      testHardLines = 800,
      moduleHardLines = 100,
      forbidWildcardImports = true,
      forbidInlineTests = true,
      forbidSubstantiveFacades = true,
      forbidPanickingProductionCode = true,
      forbidDynamicErrorSurfaces = true,
    } = policy;
    if (
      !Array.isArray(roots) ||
      roots.length === 0 ||
      roots.some((value) => typeof value !== "string" || value.length === 0)
    ) {
      fail("Rust source roots policy must be a non-empty array of non-empty strings");
    }
    if (
      !Array.isArray(generatedPrefixes) ||
      generatedPrefixes.some((value) => typeof value !== "string" || value.length === 0)
    ) {
      fail("Rust source generatedPrefixes policy must be an array of non-empty strings");
    }
    if (new Set(roots).size !== roots.length) {
      fail("Rust source roots policy must not contain duplicates");
    }
    if (new Set(generatedPrefixes).size !== generatedPrefixes.length) {
      fail("Rust source generatedPrefixes policy must not contain duplicates");
    }
    if (baselinePath !== null && (typeof baselinePath !== "string" || baselinePath.length === 0)) {
      fail("Rust source baselinePath policy must be null or a non-empty string");
    }
    for (const [name, value] of Object.entries({
      forbidWildcardImports,
      forbidInlineTests,
      forbidSubstantiveFacades,
      forbidPanickingProductionCode,
      forbidDynamicErrorSurfaces,
    })) {
      if (typeof value !== "boolean") {
        fail(`Rust source ${name} policy must be a boolean`);
      }
      if (!value) {
        fail(`Rust source ${name} policy is mandatory and cannot be disabled`);
      }
    }
    const limits = {
      productionTargetLines,
      productionHardLines,
      testTargetLines,
      testHardLines,
      moduleHardLines,
    };
    for (const [name, value] of Object.entries(limits)) {
      if (!Number.isSafeInteger(value) || value <= 0) {
        fail(`Rust source ${name} policy must be a positive safe integer`);
      }
    }
    if (
      productionTargetLines > productionHardLines ||
      testTargetLines > testHardLines ||
      moduleHardLines > productionHardLines
    ) {
      fail("Rust source line limits are inconsistent");
    }
    if (productionHardLines > MAX_PRODUCTION_SOURCE_LINES) {
      fail(
        `Rust production hard limit cannot exceed ${MAX_PRODUCTION_SOURCE_LINES} lines`,
      );
    }
    if (testHardLines > MAX_SEPARATE_TEST_SOURCE_LINES) {
      fail(
        `Rust separate-test hard limit cannot exceed ${MAX_SEPARATE_TEST_SOURCE_LINES} lines`,
      );
    }

    const normalizePolicyPath = (path, description, allowRepositoryRoot = false) => {
      const absolute = resolveRepositoryPath(path, description);
      const normalized = relative(root, absolute).replaceAll("\\", "/").replace(/\/+$/u, "");
      if (normalized.length === 0) {
        if (allowRepositoryRoot) {
          return ".";
        }
        fail(`${description} must not be the repository root`);
      }
      return normalized;
    };
    const normalizedRoots = roots.map((path) =>
      normalizePolicyPath(path, "Rust source root", true),
    );
    const normalizedGeneratedPrefixes = generatedPrefixes.map((path) =>
      normalizePolicyPath(path, "Rust generated source prefix"),
    );
    for (const sourceRoot of normalizedRoots) {
      assertRepositoryDirectory(sourceRoot, "Rust source root");
    }
    const trackedRustFiles = [...loadTrackedFiles()]
      .map((path) => path.replaceAll("\\", "/"))
      .filter((path) => path.endsWith(".rs"));
    for (const prefix of normalizedGeneratedPrefixes) {
      if (!/(?:^|\/)(?:gen|generated)(?:[./_-]|\/|$)/iu.test(prefix)) {
        fail(`Rust generated source prefix ${prefix} must identify a gen or generated path`);
      }
      if (!trackedRustFiles.some((path) => pathIsInside(path, prefix))) {
        fail(`Rust generated source prefix ${prefix} does not match a tracked Rust source file`);
      }
    }
    const uncoveredRustFiles = trackedRustFiles.filter(
      (path) =>
        !normalizedRoots.some(
          (sourceRoot) => sourceRoot === "." || pathIsInside(path, sourceRoot),
        ) &&
        !normalizedGeneratedPrefixes.some((prefix) => pathIsInside(path, prefix)),
    );
    if (uncoveredRustFiles.length !== 0) {
      fail(`Rust source roots do not govern tracked source ${uncoveredRustFiles[0]}`);
    }
    const sourceFiles = new Set(
      trackedRustFiles.filter(
        (path) =>
          normalizedRoots.some(
            (sourceRoot) => sourceRoot === "." || pathIsInside(path, sourceRoot),
          ) &&
          !normalizedGeneratedPrefixes.some((prefix) => pathIsInside(path, prefix)),
      ),
    );
    if (sourceFiles.size === 0) {
      fail("Rust source policy found no Rust source files");
    }

    const baseline = new Map();
    if (baselinePath !== null) {
      const normalizedBaselinePath = normalizePolicyPath(
        baselinePath,
        "Rust source baseline path",
      );
      for (const [index, line] of readText(normalizedBaselinePath).split("\n").entries()) {
        const trimmed = line.trim();
        if (trimmed.length === 0 || trimmed.startsWith("#")) {
          continue;
        }
        const fields = line.split("\t");
        if (fields.length !== 2 || !/^[1-9][0-9]*$/u.test(fields[1])) {
          fail(`Rust source baseline line ${index + 1} must be path<TAB>positive-lines`);
        }
        const path = normalizePolicyPath(fields[0], `Rust source baseline line ${index + 1} path`);
        const allowedLines = Number(fields[1]);
        if (!Number.isSafeInteger(allowedLines)) {
          fail(`Rust source baseline line ${index + 1} exceeds the safe integer range`);
        }
        if (baseline.has(path)) {
          fail(`Rust source baseline path ${path} is duplicated`);
        }
        if (!sourceFiles.has(path)) {
          fail(`Rust source baseline path ${path} is not a governed Rust source file`);
        }
        baseline.set(path, allowedLines);
      }
    }

    const wildcardImport = /^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?use\b[^;]*\*[^;]*;/mu;
    const externalTestModule = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:#\s*\[[^\]]+\]\s*)*(?:pub(?:\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*;/gu;
    const testConfiguration = /#\s*\[\s*cfg\s*\([^\]]*\btest\b[^\]]*\)\s*\]/u;
    const testAttribute = /#\s*\[\s*(?:[A-Za-z_][A-Za-z0-9_]*::)*test(?:\s*\([^\]]*\))?\s*\]/u;
    const substantiveFacade = /(?:^|\n)[ \t]*(?:(?:pub(?:\([^)]*\))?|async|unsafe|const|extern(?:[ \t]+"[^"]*")?)[ \t]+)*(?:fn|struct|enum|union|trait|impl|static|const)[ \t]+|(?:^|\n)[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?mod[ \t]+[A-Za-z_][A-Za-z0-9_]*[ \t]*\{|(?:^|\n)[ \t]*macro_rules[ \t]*!/mu;
    const panickingProductionPatterns = [
      ["unwrap", /\.unwrap[ \t\n]*\(/u],
      ["expect", /\.expect[ \t\n]*\(/u],
      ["panic macro", /\bpanic[ \t\n]*!/u],
      ["todo macro", /\btodo[ \t\n]*!/u],
      ["unimplemented macro", /\bunimplemented[ \t\n]*!/u],
      ["unreachable macro", /\bunreachable[ \t\n]*!/u],
      ["assert macro", /\b(?:debug_)?assert(?:_eq|_ne)?[ \t\n]*!/u],
    ];
    const dynamicErrorPatterns = [
      ["anyhow error", /\banyhow(?:::|[ \t\n]*!)/u],
      [
        "boxed dynamic error",
        /\bBox[ \t\n]*<[ \t\n]*dyn[ \t\n]+(?:(?:std|core)::error::)?Error\b/u,
      ],
      [
        "string Result error",
        /\bResult[ \t\n]*<[^;={}]*,[ \t\n]*(?:String|&[ \t]*(?:\x27static[ \t]+)?str)[ \t\n]*>/u,
      ],
      [
        "string error alias",
        /\btype[ \t]+(?:[A-Za-z_][A-Za-z0-9_]*)?Error(?:Reason)?[ \t]*=[ \t]*(?:String|&[ \t]*(?:\x27static[ \t]+)?str)\b/u,
      ],
    ];
    const forbiddenErrorField = /\bString\b|&[ \t]*(?:\x27static[ \t]+)?str\b|\bBox[ \t\n]*<[ \t\n]*dyn[ \t\n]+(?:(?:std|core)::error::)?Error\b/u;
    const assertTypedErrorDefinitions = (path, source) => {
      const declaration = /\b(?:enum|struct)[ \t]+((?:[A-Za-z_][A-Za-z0-9_]*)?Error(?:Reason)?)\b[^;{]*\{/gu;
      let match = declaration.exec(source);
      while (match !== null) {
        let depth = 1;
        let cursor = declaration.lastIndex;
        while (cursor < source.length && depth > 0) {
          if (source[cursor] === "{") {
            depth += 1;
          } else if (source[cursor] === "}") {
            depth -= 1;
          }
          cursor += 1;
        }
        if (depth !== 0) {
          fail(`${path} error definition ${match[1]} has unbalanced braces`);
        }
        const body = source.slice(declaration.lastIndex, cursor - 1);
        if (forbiddenErrorField.test(body)) {
          fail(`${path} error definition ${match[1]} contains a dynamic or string field`);
        }
        declaration.lastIndex = cursor;
        match = declaration.exec(source);
      }
      const tupleDeclaration = /\bstruct[ \t]+((?:[A-Za-z_][A-Za-z0-9_]*)?Error(?:Reason)?)[ \t]*\(([^;]*)\)[ \t]*;/gu;
      for (const tupleMatch of source.matchAll(tupleDeclaration)) {
        if (forbiddenErrorField.test(tupleMatch[2])) {
          fail(`${path} error definition ${tupleMatch[1]} contains a dynamic or string field`);
        }
      }
    };
    const lineCount = (text) => {
      if (text.length === 0) {
        return 0;
      }
      const lines = text.split("\n").length;
      return text.endsWith("\n") ? lines - 1 : lines;
    };
    for (const path of [...sourceFiles].sort()) {
      const text = readText(path);
      const executableSource = scrubRustCommentsAndStrings(text);
      if (forbidWildcardImports && wildcardImport.test(executableSource)) {
        fail(`${path} must not use a wildcard import or re-export`);
      }
      const lines = lineCount(text);
      const fileName = path.slice(path.lastIndexOf("/") + 1);
      const isTest =
        path.includes("/tests/") ||
        fileName === "test.rs" ||
        fileName === "tests.rs" ||
        fileName.endsWith("_test.rs") ||
        fileName.endsWith("_tests.rs");
      if (!isTest && forbidInlineTests) {
        const withoutExternalTestModules = executableSource.replace(externalTestModule, "");
        if (
          testConfiguration.test(withoutExternalTestModules) ||
          testAttribute.test(withoutExternalTestModules)
        ) {
          fail(`${path} must keep test implementations in a separate test file`);
        }
      }
      if (
        !isTest &&
        forbidSubstantiveFacades &&
        (fileName === "lib.rs" || fileName === "mod.rs") &&
        substantiveFacade.test(executableSource)
      ) {
        fail(`${path} must remain a declaration-and-re-export-only facade`);
      }
      if (!isTest && forbidPanickingProductionCode) {
        for (const [description, pattern] of panickingProductionPatterns) {
          if (pattern.test(executableSource)) {
            fail(`${path} contains forbidden production ${description}`);
          }
        }
      }
      if (!isTest && forbidDynamicErrorSurfaces) {
        assertTypedErrorDefinitions(path, executableSource);
        for (const [description, pattern] of dynamicErrorPatterns) {
          if (pattern.test(executableSource)) {
            fail(`${path} contains forbidden ${description}`);
          }
        }
      }
      if (
        !isTest &&
        (fileName === "lib.rs" || fileName === "mod.rs") &&
        lines > moduleHardLines
      ) {
        fail(`${path} has ${lines} lines, exceeding the module hard limit ${moduleHardLines}`);
      }
      const target = isTest ? testTargetLines : productionTargetLines;
      const hardLimit = isTest ? testHardLines : productionHardLines;
      if (lines > hardLimit) {
        fail(`${path} has ${lines} lines, exceeding its hard limit ${hardLimit}`);
      }
      const allowedLines = baseline.get(path);
      if (lines > target && allowedLines === undefined) {
        fail(`${path} has ${lines} lines and requires a shrinking-only baseline above ${target}`);
      }
      if (allowedLines !== undefined && allowedLines <= target) {
        fail(`${path} baseline ${allowedLines} is stale because it is not above target ${target}`);
      }
      if (allowedLines !== undefined && lines > allowedLines) {
        fail(`${path} grew from its baseline ${allowedLines} to ${lines} lines`);
      }
      if (allowedLines !== undefined && lines <= target) {
        fail(`${path} baseline is stale because the file is now within target ${target}`);
      }
    }
    reportSourcePolicy("rust");
  };

  const assertLanguageVerificationPolicy = (language, verification, requiredRoles) => {
    if (!Array.isArray(verification) || verification.length === 0) {
      fail(`${language} verification policy requires at least one command`);
    }
    const allowedRoles = new Set(requiredRoles);
    const observedRoles = new Set();
    for (const [index, entry] of verification.entries()) {
      const { roles, command, args, options = {} } = entry ?? {};
      if (
        !Array.isArray(roles) ||
        roles.length === 0 ||
        roles.some((role) => typeof role !== "string" || !allowedRoles.has(role))
      ) {
        fail(`${language} verification command ${index} has invalid roles`);
      }
      if (new Set(roles).size !== roles.length) {
        fail(`${language} verification command ${index} has duplicate roles`);
      }
      for (const role of roles) {
        if (observedRoles.has(role)) {
          fail(`${language} verification role ${role} is configured more than once`);
        }
        observedRoles.add(role);
      }
      if (typeof command !== "string" || command.length === 0) {
        fail(`${language} verification command ${index} requires a command`);
      }
      if (!Array.isArray(args) || args.some((arg) => typeof arg !== "string")) {
        fail(`${language} verification command ${index} arguments must be strings`);
      }
      if (options === null || typeof options !== "object" || Array.isArray(options)) {
        fail(`${language} verification command ${index} options must be an object`);
      }
    }
    for (const role of requiredRoles) {
      if (!observedRoles.has(role)) {
        fail(`${language} verification policy is missing the ${role} role`);
      }
    }
    for (const entry of verification) {
      run(entry.command, entry.args, entry.options ?? {});
    }
  };

  const createSourcePolicyState = ({
    language,
    policy,
    extensions,
    isTestPath,
    inferredFacadePath = () => false,
    isSourcePath = () => true,
  }) => {
    const {
      roots = ["."],
      generatedPrefixes = [],
      facadeFiles = [],
      baselinePath = null,
      productionTargetLines = 500,
      productionHardLines = 500,
      testTargetLines = 800,
      testHardLines = 800,
      facadeHardLines = 100,
    } = policy;
    for (const [name, values, allowEmpty] of [
      ["roots", roots, false],
      ["generatedPrefixes", generatedPrefixes, true],
      ["facadeFiles", facadeFiles, true],
    ]) {
      if (
        !Array.isArray(values) ||
        (!allowEmpty && values.length === 0) ||
        values.some((value) => typeof value !== "string" || value.length === 0)
      ) {
        fail(`${language} source ${name} policy must be an array of non-empty strings`);
      }
      if (new Set(values).size !== values.length) {
        fail(`${language} source ${name} policy must not contain duplicates`);
      }
    }
    if (baselinePath !== null && (typeof baselinePath !== "string" || baselinePath.length === 0)) {
      fail(`${language} source baselinePath policy must be null or a non-empty string`);
    }
    const limits = {
      productionTargetLines,
      productionHardLines,
      testTargetLines,
      testHardLines,
      facadeHardLines,
    };
    for (const [name, value] of Object.entries(limits)) {
      if (!Number.isSafeInteger(value) || value <= 0) {
        fail(`${language} source ${name} policy must be a positive safe integer`);
      }
    }
    if (
      productionTargetLines > productionHardLines ||
      testTargetLines > testHardLines ||
      facadeHardLines > productionHardLines
    ) {
      fail(`${language} source line limits are inconsistent`);
    }
    if (productionHardLines > MAX_PRODUCTION_SOURCE_LINES) {
      fail(
        `${language} production hard limit cannot exceed ${MAX_PRODUCTION_SOURCE_LINES} lines`,
      );
    }
    if (testHardLines > MAX_SEPARATE_TEST_SOURCE_LINES) {
      fail(
        `${language} separate-test hard limit cannot exceed ${MAX_SEPARATE_TEST_SOURCE_LINES} lines`,
      );
    }

    const normalizePath = (path, description, allowRepositoryRoot = false) => {
      const absolute = resolveRepositoryPath(path, description);
      const normalized = relative(root, absolute).replaceAll("\\", "/").replace(/\/+$/u, "");
      if (normalized.length === 0) {
        if (allowRepositoryRoot) {
          return ".";
        }
        fail(`${description} must not be the repository root`);
      }
      return normalized;
    };
    const normalizedRoots = roots.map((path) =>
      normalizePath(path, `${language} source root`, true),
    );
    const normalizedGeneratedPrefixes = generatedPrefixes.map((path) =>
      normalizePath(path, `${language} generated source prefix`),
    );
    for (const sourceRoot of normalizedRoots) {
      assertRepositoryDirectory(sourceRoot, `${language} source root`);
    }
    const trackedLanguageFiles = [...loadTrackedFiles()]
      .map((path) => path.replaceAll("\\", "/"))
      .filter(
        (path) =>
          extensions.some((extension) => path.endsWith(extension)) && isSourcePath(path),
      );
    for (const prefix of normalizedGeneratedPrefixes) {
      if (!/(?:^|\/)(?:gen|generated)(?:[./_-]|\/|$)/iu.test(prefix)) {
        fail(
          `${language} generated source prefix ${prefix} must identify a gen or generated path`,
        );
      }
      if (!trackedLanguageFiles.some((path) => pathIsInside(path, prefix))) {
        fail(
          `${language} generated source prefix ${prefix} does not match a tracked source file`,
        );
      }
    }
    const uncoveredLanguageFiles = trackedLanguageFiles.filter(
      (path) =>
        !normalizedRoots.some(
          (sourceRoot) => sourceRoot === "." || pathIsInside(path, sourceRoot),
        ) &&
        !normalizedGeneratedPrefixes.some((prefix) => pathIsInside(path, prefix)),
    );
    if (uncoveredLanguageFiles.length !== 0) {
      fail(
        `${language} source roots do not govern tracked source ${uncoveredLanguageFiles[0]}`,
      );
    }
    const sourceFiles = trackedLanguageFiles
      .filter(
        (path) =>
          normalizedRoots.some(
            (sourceRoot) => sourceRoot === "." || pathIsInside(path, sourceRoot),
          ) &&
          !normalizedGeneratedPrefixes.some((prefix) => pathIsInside(path, prefix)),
      )
      .sort();
    if (sourceFiles.length === 0) {
      fail(`${language} source policy found no governed source files`);
    }
    const sourceFileSet = new Set(sourceFiles);
    const normalizedFacadeFiles = new Set(
      facadeFiles.map((path) => normalizePath(path, `${language} facade file`)),
    );
    for (const path of normalizedFacadeFiles) {
      if (!sourceFileSet.has(path)) {
        fail(`${language} facade file ${path} is not a governed source file`);
      }
    }

    const baseline = new Map();
    if (baselinePath !== null) {
      const normalizedBaselinePath = normalizePath(
        baselinePath,
        `${language} source baseline path`,
      );
      for (const [index, line] of readText(normalizedBaselinePath).split("\n").entries()) {
        const trimmed = line.trim();
        if (trimmed.length === 0 || trimmed.startsWith("#")) {
          continue;
        }
        const fields = line.split("\t");
        if (fields.length !== 2 || !/^[1-9][0-9]*$/u.test(fields[1])) {
          fail(`${language} source baseline line ${index + 1} must be path<TAB>positive-lines`);
        }
        const path = normalizePath(
          fields[0],
          `${language} source baseline line ${index + 1} path`,
        );
        const allowedLines = Number(fields[1]);
        if (!Number.isSafeInteger(allowedLines)) {
          fail(`${language} source baseline line ${index + 1} exceeds the safe integer range`);
        }
        if (baseline.has(path)) {
          fail(`${language} source baseline path ${path} is duplicated`);
        }
        if (!sourceFileSet.has(path)) {
          fail(`${language} source baseline path ${path} is not a governed source file`);
        }
        baseline.set(path, allowedLines);
      }
    }

    const lineCount = (text) => {
      if (text.length === 0) {
        return 0;
      }
      const lines = text.split("\n").length;
      return text.endsWith("\n") ? lines - 1 : lines;
    };
    const assertFileLimits = (path, text) => {
      const isTest = isTestPath(path);
      const lines = lineCount(text);
      const isFacade = normalizedFacadeFiles.has(path) || inferredFacadePath(path);
      if (!isTest && isFacade && lines > facadeHardLines) {
        fail(`${path} has ${lines} lines, exceeding the facade hard limit ${facadeHardLines}`);
      }
      const target = isTest ? testTargetLines : productionTargetLines;
      const hardLimit = isTest ? testHardLines : productionHardLines;
      if (lines > hardLimit) {
        fail(`${path} has ${lines} lines, exceeding its hard limit ${hardLimit}`);
      }
      const allowedLines = baseline.get(path);
      if (lines > target && allowedLines === undefined) {
        fail(`${path} has ${lines} lines and requires a shrinking-only baseline above ${target}`);
      }
      if (allowedLines !== undefined && allowedLines <= target) {
        fail(`${path} baseline ${allowedLines} is stale because it is not above target ${target}`);
      }
      if (allowedLines !== undefined && lines > allowedLines) {
        fail(`${path} grew from its baseline ${allowedLines} to ${lines} lines`);
      }
      if (allowedLines !== undefined && lines <= target) {
        fail(`${path} baseline is stale because the file is now within target ${target}`);
      }
      return { isFacade, isTest };
    };
    return { assertFileLimits, sourceFiles };
  };

  const assertBooleanPolicyOptions = (language, options) => {
    for (const [name, value] of Object.entries(options)) {
      if (typeof value !== "boolean") {
        fail(`${language} source ${name} policy must be a boolean`);
      }
      if (!value) {
        fail(`${language} source ${name} policy is mandatory and cannot be disabled`);
      }
    }
  };

  const assertLanguageConfigurationPolicy = (language, configuration) => {
    if (
      configuration === null ||
      typeof configuration !== "object" ||
      Array.isArray(configuration)
    ) {
      fail(`${language} source policy requires a configuration text policy`);
    }
    const files = configuration.files;
    if (
      !Array.isArray(files) ||
      files.length === 0 ||
      files.some((entry) => {
        const requiredCount = Array.isArray(entry?.required) ? entry.required.length : 0;
        const requiredMatchCount = Array.isArray(entry?.requiredMatches)
          ? entry.requiredMatches.length
          : 0;
        return requiredCount + requiredMatchCount === 0;
      })
    ) {
      fail(`${language} configuration policy must require evidence from every file`);
    }
    assertTextPolicy(configuration);
  };

  const assertTypeScriptSourcePolicy = (policy = {}) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("TypeScript source policy must be an object");
    }
    const {
      tsconfigPaths = ["tsconfig.json"],
      staticAnalysis,
      verification,
      forbidAny = true,
      forbidSuppressions = true,
      forbidUnsafeAssertions = true,
      forbidInlineTests = true,
      forbidWildcardSurfaces = true,
      forbidGenericErrors = true,
      forbidSubstantiveFacades = true,
    } = policy;
    assertBooleanPolicyOptions("TypeScript", {
      forbidAny,
      forbidSuppressions,
      forbidUnsafeAssertions,
      forbidInlineTests,
      forbidWildcardSurfaces,
      forbidGenericErrors,
      forbidSubstantiveFacades,
    });
    if (
      !Array.isArray(tsconfigPaths) ||
      tsconfigPaths.length === 0 ||
      tsconfigPaths.some((path) => typeof path !== "string" || path.length === 0) ||
      new Set(tsconfigPaths).size !== tsconfigPaths.length
    ) {
      fail("TypeScript tsconfigPaths policy must be a non-empty array of unique paths");
    }
    const requiredCompilerOptions = [
      "strict",
      "noImplicitAny",
      "noUncheckedIndexedAccess",
      "exactOptionalPropertyTypes",
      "useUnknownInCatchVariables",
      "noImplicitOverride",
      "noFallthroughCasesInSwitch",
    ];
    for (const path of tsconfigPaths) {
      let configuration;
      try {
        const withoutComments = scrubJavaScriptCommentsAndStrings(readText(path), {
          preserveStrings: true,
        });
        configuration = JSON.parse(withoutComments.replace(/,\s*([}\]])/gu, "$1"));
      } catch {
        fail(`${path} is not valid JSON-with-comments`);
      }
      if (
        configuration === null ||
        typeof configuration !== "object" ||
        Array.isArray(configuration) ||
        configuration.compilerOptions === null ||
        typeof configuration.compilerOptions !== "object" ||
        Array.isArray(configuration.compilerOptions)
      ) {
        fail(`${path} must define compilerOptions`);
      }
      for (const option of requiredCompilerOptions) {
        if (configuration.compilerOptions[option] !== true) {
          fail(`${path} compilerOptions.${option} must be explicitly true`);
        }
      }
    }
    assertLanguageConfigurationPolicy("TypeScript static analysis", staticAnalysis);
    const isTypeScriptTest = (path) =>
      /(?:^|\/)(?:test|tests|__tests__)\//u.test(path) ||
      /(?:\.(?:test|spec)|_(?:test|tests))\.(?:cts|mts|tsx?|ts)$/u.test(path);
    const isTypeScriptFacade = (path) => /(?:^|\/)index\.(?:cts|mts|tsx?|ts)$/u.test(path);
    const state = createSourcePolicyState({
      language: "TypeScript",
      policy,
      extensions: [".ts", ".tsx", ".mts", ".cts"],
      isTestPath: isTypeScriptTest,
      inferredFacadePath: isTypeScriptFacade,
    });
    const wildcardSurface = /(?:^|\n)[ \t]*(?:export[ \t]+\*(?:[ \t]+from)?|import[ \t]+\*[ \t]+as\b)/mu;
    const inlineTest = /(?:^|[^A-Za-z0-9_$.])(?:describe|it|test)[ \t\n]*\(/u;
    const substantiveFacade = /(?:^|\n)[ \t]*(?:export[ \t]+)?(?:async[ \t]+)?(?:function|class|enum|namespace|const|let|var)\b/mu;
    const nonNullAssertion = /[A-Za-z0-9_)\]}][ \t]*!(?!=)/u;
    const unsafeAssertion = /\bas[ \t\n]+(?:any|unknown|never)\b/u;
    const genericError = /\bthrow[ \t\n]+new[ \t\n]+Error[ \t\n]*\(|\bPromise\.reject[ \t\n]*\([ \t\n]*new[ \t\n]+Error[ \t\n]*\(/u;
    const stringFailure = /\bthrow[ \t\n]*["'`]|\bPromise\.reject[ \t\n]*\([ \t\n]*["'`]/u;
    for (const path of state.sourceFiles) {
      const text = readText(path);
      const source = scrubJavaScriptCommentsAndStrings(text);
      const commentFreeSource = scrubJavaScriptCommentsAndStrings(text, {
        preserveStrings: true,
      });
      const directiveSource = scrubJavaScriptCommentsAndStrings(text, {
        preserveComments: true,
      });
      const { isFacade, isTest } = state.assertFileLimits(path, text);
      const hasForbiddenSuppression =
        /\/\/[^\n]*(?:@ts-ignore\b|eslint-(?:disable|disable-line|disable-next-line)\b)/u.test(directiveSource) ||
        /\/\*[\s\S]*?(?:@ts-ignore\b|eslint-(?:disable|disable-line|disable-next-line)\b)[\s\S]*?\*\//u.test(directiveSource);
      if (forbidSuppressions && hasForbiddenSuppression) {
        fail(`${path} contains a forbidden TypeScript or ESLint suppression`);
      }
      for (const line of directiveSource.split("\n")) {
        const directive = /^[ \t]*\/\/[ \t]*@ts-expect-error\b(.*)$/u.exec(line);
        if (directive === null) {
          continue;
        }
        if (!isTest) {
          fail(`${path} may use @ts-expect-error only in a separate test file`);
        }
        if (!/^[ \t]*(?::|--)[ \t]+\S/u.test(directive[1])) {
          fail(`${path} @ts-expect-error requires an explanation`);
        }
      }
      if (forbidAny && /\bany\b/u.test(source)) {
        fail(`${path} contains forbidden TypeScript any`);
      }
      if (forbidWildcardSurfaces && wildcardSurface.test(source)) {
        fail(`${path} contains a wildcard import or export`);
      }
      if (!isTest && forbidInlineTests && inlineTest.test(source)) {
        fail(`${path} must keep test implementations in a separate test file`);
      }
      if (!isTest && forbidUnsafeAssertions && (unsafeAssertion.test(source) || nonNullAssertion.test(source))) {
        fail(`${path} contains a forbidden unsafe TypeScript assertion`);
      }
      if (!isTest && forbidGenericErrors && (genericError.test(source) || stringFailure.test(commentFreeSource))) {
        fail(`${path} contains a forbidden untyped TypeScript failure`);
      }
      if (!isTest && isFacade && forbidSubstantiveFacades && substantiveFacade.test(source)) {
        fail(`${path} must remain an explicit import, export, and type-only facade`);
      }
    }
    assertLanguageVerificationPolicy(
      "TypeScript",
      verification,
      ["typecheck", "lint", "test"],
    );
    reportSourcePolicy("typescript");
  };

  const assertSwiftSourcePolicy = (policy = {}) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("Swift source policy must be an object");
    }
    const {
      configuration,
      verification,
      forbidSuppressions = true,
      forbidUnsafeOperations = true,
      forbidInlineTests = true,
      forbidGenericErrors = true,
      forbidSubstantiveFacades = true,
      requireTypedThrows = true,
      forbidConcurrencyEscapeHatches = true,
    } = policy;
    assertBooleanPolicyOptions("Swift", {
      forbidSuppressions,
      forbidUnsafeOperations,
      forbidInlineTests,
      forbidGenericErrors,
      forbidSubstantiveFacades,
      requireTypedThrows,
      forbidConcurrencyEscapeHatches,
    });
    assertLanguageConfigurationPolicy("Swift", configuration);
    const isSwiftTest = (path) =>
      /(?:^|\/)(?:Tests|[A-Za-z0-9_-]+Tests)\//u.test(path);
    const state = createSourcePolicyState({
      language: "Swift",
      policy,
      extensions: [".swift"],
      isTestPath: isSwiftTest,
      // Package.swift is a build manifest validated through configuration
      // policy, not an implementation module subject to source layout rules.
      isSourcePath: (path) => !/(?:^|\/)Package\.swift$/u.test(path),
    });
    const unsafeOperation = /\b(?:try|as)[ \t]*!|[A-Za-z0-9_)\]}][ \t]*!(?!=)/u;
    const crashOperation = /\b(?:fatalError|preconditionFailure|assertionFailure)[ \t\n]*\(/u;
    const inlineTest = /\bXCTestCase\b|@[A-Za-z0-9_.]*Test\b|\bfunc[ \t]+test[A-Z_]/u;
    const genericError = /\bthrow[ \t\n]+NSError[ \t\n]*\(|\bResult[ \t\n]*<[^>]+,[ \t\n]*(?:any[ \t\n]+)?Error[ \t\n]*>/u;
    const untypedThrows = /\bthrows\b(?![ \t\n]*\()/u;
    const substantiveFacade = /(?:^|\n)[ \t]*(?:(?:public|package|internal|private|fileprivate|open|final|indirect|nonisolated|isolated|distributed|static|class|mutating|nonmutating|override|required|convenience)[ \t]+)*(?:func|class|struct|enum|actor|protocol|extension|let|var)\b/mu;
    for (const path of state.sourceFiles) {
      const text = readText(path);
      const source = scrubSlashCommentsAndStrings(text);
      const directiveSource = scrubSlashCommentsAndStrings(text, {
        preserveComments: true,
      });
      const { isFacade, isTest } = state.assertFileLimits(path, text);
      const hasSwiftLintSuppression =
        /\/\/[^\n]*swiftlint[ \t]*:[ \t]*disable\b/u.test(directiveSource) ||
        /\/\*[\s\S]*?swiftlint[ \t]*:[ \t]*disable\b[\s\S]*?\*\//u.test(directiveSource);
      if (forbidSuppressions && hasSwiftLintSuppression) {
        fail(`${path} contains a forbidden SwiftLint suppression`);
      }
      if (!isTest && forbidInlineTests && inlineTest.test(source)) {
        fail(`${path} must keep test implementations in a separate test file`);
      }
      if (!isTest && forbidUnsafeOperations && (unsafeOperation.test(source) || crashOperation.test(source))) {
        fail(`${path} contains a forbidden unsafe or terminating Swift operation`);
      }
      if (!isTest && forbidGenericErrors && genericError.test(source)) {
        fail(`${path} contains a forbidden generic Swift error surface`);
      }
      if (!isTest && requireTypedThrows && untypedThrows.test(source)) {
        fail(`${path} must use typed throws or a typed Result`);
      }
      if (
        !isTest &&
        forbidConcurrencyEscapeHatches &&
        /@unchecked[ \t]+Sendable\b|@preconcurrency\b/u.test(source)
      ) {
        fail(`${path} contains a forbidden Swift concurrency escape hatch`);
      }
      if (!isTest && isFacade && forbidSubstantiveFacades && substantiveFacade.test(source)) {
        fail(`${path} must remain a declaration-and-re-export-only Swift facade`);
      }
    }
    assertLanguageVerificationPolicy(
      "Swift",
      verification,
      ["format", "lint", "build", "test"],
    );
    reportSourcePolicy("swift");
  };

  const assertKotlinSourcePolicy = (policy = {}) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("Kotlin source policy must be an object");
    }
    const {
      configuration,
      verification,
      forbidSuppressions = true,
      forbidUnsafeOperations = true,
      forbidInlineTests = true,
      forbidWildcardImports = true,
      forbidGenericErrors = true,
      forbidSubstantiveFacades = true,
      forbidLateinit = true,
    } = policy;
    assertBooleanPolicyOptions("Kotlin", {
      forbidSuppressions,
      forbidUnsafeOperations,
      forbidInlineTests,
      forbidWildcardImports,
      forbidGenericErrors,
      forbidSubstantiveFacades,
      forbidLateinit,
    });
    assertLanguageConfigurationPolicy("Kotlin", configuration);
    const isKotlinTest = (path) =>
      /(?:^|\/)src\/(?:test|androidTest|commonTest|[A-Za-z0-9_]+Test)\//u.test(path);
    const state = createSourcePolicyState({
      language: "Kotlin",
      policy,
      // Gradle Kotlin scripts are build configuration and are validated by the
      // configuration policy. Authored Kotlin implementation uses .kt files.
      extensions: [".kt"],
      isTestPath: isKotlinTest,
    });
    const wildcardImport = /(?:^|\n)[ \t]*import[ \t]+[^\n;]*\.\*[ \t]*(?:;|$)/mu;
    const inlineTest = /@(Test|ParameterizedTest|RepeatedTest|TestFactory)\b|\b(?:kotlin\.test|org\.junit)\b/u;
    const unsafeOperation = /!!/u;
    const terminatingOperation = /\b(?:error|TODO|check|checkNotNull|require|requireNotNull)[ \t\n]*\(/u;
    const genericError = /\b(?:Exception|RuntimeException|IllegalArgumentException|IllegalStateException)[ \t\n]*\(/u;
    const substantiveFacade = /(?:^|\n)[ \t]*(?:(?:public|internal|private|protected|expect|actual|final|open|abstract|sealed|data|value|inline|suspend|operator|infix|tailrec|external|const|lateinit)[ \t]+)*(?:fun|class|interface|object|enum[ \t]+class|typealias|val|var)\b/mu;
    for (const path of state.sourceFiles) {
      const text = readText(path);
      const source = scrubSlashCommentsAndStrings(text);
      const { isFacade, isTest } = state.assertFileLimits(path, text);
      if (forbidSuppressions && /@Suppress[ \t\n]*\(/u.test(source)) {
        fail(`${path} contains a forbidden Kotlin suppression`);
      }
      if (forbidWildcardImports && wildcardImport.test(source)) {
        fail(`${path} contains a wildcard Kotlin import`);
      }
      if (!isTest && forbidInlineTests && inlineTest.test(source)) {
        fail(`${path} must keep test implementations in a separate test file`);
      }
      if (!isTest && forbidUnsafeOperations) {
        if (unsafeOperation.test(source) || terminatingOperation.test(source)) {
          fail(`${path} contains a forbidden unsafe or terminating Kotlin operation`);
        }
        for (const line of source.split("\n")) {
          if (!/^[ \t]*import\b/u.test(line) && /\bas[ \t]+(?!\?)/u.test(line)) {
            fail(`${path} contains a forbidden unsafe Kotlin cast`);
          }
        }
      }
      if (!isTest && forbidGenericErrors && genericError.test(source)) {
        fail(`${path} contains a forbidden generic Kotlin error`);
      }
      if (!isTest && forbidLateinit && /\blateinit\b/u.test(source)) {
        fail(`${path} contains forbidden Kotlin lateinit state`);
      }
      if (!isTest && isFacade && forbidSubstantiveFacades && substantiveFacade.test(source)) {
        fail(`${path} must remain a declaration-and-re-export-only Kotlin facade`);
      }
    }
    assertLanguageVerificationPolicy(
      "Kotlin",
      verification,
      ["format", "static-analysis", "compile", "test"],
    );
    reportSourcePolicy("kotlin");
  };

  const snapshotDirectory = (path) => {
    const directory = assertRepositoryDirectory(path);
    const snapshot = new Map();
    const visit = (current) => {
      let entries;
      try {
        entries = readdirSync(current).sort();
      } catch {
        fail(`${relative(root, current)} is missing or inaccessible`);
      }
      for (const entry of entries) {
        const absolute = resolve(current, entry);
        const status = lstatSync(absolute);
        if (status.isSymbolicLink()) {
          fail(`${relative(root, absolute)} must not be a symbolic link`);
        }
        if (status.isDirectory()) {
          visit(absolute);
        } else if (status.isFile()) {
          const file = relative(directory, absolute);
          snapshot.set(file, fingerprintFile(`${path}/${file}`));
        } else {
          fail(`${relative(root, absolute)} is not a regular file`);
        }
      }
    };
    visit(directory);

    if (requireTrackedFiles) {
      const prefix = `${path}/`;
      const tracked = new Set(
        listFiles(path).map((file) => file.slice(prefix.length)),
      );
      for (const file of snapshot.keys()) {
        if (!tracked.has(file)) {
          fail(`${path}/${file} is not tracked by Git`);
        }
      }
      for (const file of tracked) {
        if (!snapshot.has(file)) {
          fail(`${path}/${file} is tracked by Git but missing from the worktree`);
        }
      }
    }

    return snapshot;
  };

  const assertSnapshotsEqual = (path, before, after) => {
    if (before.size !== after.size) {
      fail(`${path} changed file count after regeneration`);
    }

    for (const [file, contents] of before) {
      const regenerated = after.get(file);
      if (regenerated === undefined || contents !== regenerated) {
        fail(`${path}/${file} is stale; run the protobuf generation and hardening steps`);
      }
    }
  };

  const pathIsInside = (path, directory) =>
    path === directory || path.startsWith(`${directory}/`);

  const snapshotRepositoryFilesOutside = (excludedPaths) => {
    const excluded = excludedPaths.map((path) => {
      resolveRepositoryPath(path);
      return path.replace(/\/+$/u, "");
    });
    const files = new Set([...loadTrackedFiles(), ...loadUntrackedFiles()]);
    const snapshot = new Map();
    for (const file of files) {
      if (excluded.some((path) => pathIsInside(file, path))) {
        continue;
      }
      snapshot.set(file, fingerprintFile(file));
    }
    return snapshot;
  };

  const assertRepositorySnapshotsEqual = (before, after) => {
    if (before.size !== after.size) {
      fail("protobuf regeneration changed files outside the declared generated paths");
    }
    for (const [file, contents] of before) {
      const regenerated = after.get(file);
      if (regenerated === undefined || contents !== regenerated) {
        fail(`protobuf regeneration modified ${file} outside the declared generated paths`);
      }
    }
  };

  const validateGeneratedArtifactsPolicy = (regeneration) => {
    const generatedPaths = regeneration?.generatedPaths ?? [];
    const commands = regeneration?.commands ?? [];
    if (!Array.isArray(generatedPaths) || generatedPaths.length === 0) {
      fail("generated artifact freshness check requires at least one generated path");
    }
    if (generatedPaths.some((path) => typeof path !== "string" || path.length === 0)) {
      fail("generated artifact paths must be non-empty strings");
    }
    if (!Array.isArray(commands) || commands.length === 0) {
      fail("generated artifact freshness check requires at least one regeneration command");
    }
    const normalizedPaths = generatedPaths.map((path) =>
      relative(root, assertRepositoryDirectory(path, "generated artifact path")).replaceAll(
        "\\",
        "/",
      ),
    );
    if (normalizedPaths.some((path) => path.length === 0 || path === ".")) {
      fail("generated artifact paths must not include the repository root");
    }
    for (const [index, path] of normalizedPaths.entries()) {
      if (
        normalizedPaths.some(
          (candidate, candidateIndex) =>
            candidateIndex !== index && pathIsInside(path, candidate),
        )
      ) {
        fail("generated artifact paths must not overlap");
      }
    }
    for (const entry of commands) {
      if (!Array.isArray(entry) || entry.length < 2 || entry.length > 3) {
        fail("regeneration commands must be [command, args, options?] tuples");
      }
      const [command, args] = entry;
      if (
        typeof command !== "string" ||
        command.length === 0 ||
        !Array.isArray(args) ||
        args.some((arg) => typeof arg !== "string")
      ) {
        fail("regeneration commands require a command and string arguments");
      }
      const options = entry[2];
      if (
        options !== undefined &&
        (options === null || typeof options !== "object" || Array.isArray(options))
      ) {
        fail("regeneration command options must be an object");
      }
    }
    return { generatedPaths: normalizedPaths, commands };
  };

  const assertGeneratedArtifactsFresh = (regeneration) => {
    const { generatedPaths, commands } = validateGeneratedArtifactsPolicy(regeneration);

    const snapshotsBefore = new Map(
      generatedPaths.map((path) => [path, snapshotDirectory(path)]),
    );
    const repositoryBefore = requireTrackedFiles
      ? snapshotRepositoryFilesOutside(generatedPaths)
      : null;
    runCommands(commands);
    for (const path of generatedPaths) {
      assertSnapshotsEqual(path, snapshotsBefore.get(path), snapshotDirectory(path));
    }
    if (repositoryBefore !== null) {
      assertRepositorySnapshotsEqual(
        repositoryBefore,
        snapshotRepositoryFilesOutside(generatedPaths),
      );
    }
  };

  const assertGeneratedProtoHardeningPolicy = (policy) => {
    const {
      hardeningScript,
      protoSchema,
      generatedRust,
      generatedView,
      protoCargo,
      workflow,
      workflowStepName,
      workflowStepRun,
      requiredScriptNeedles = [],
      forbiddenScriptNeedles = [],
      requiredGeneratedNeedles = [],
      forbiddenGeneratedNeedles = [],
      requiredViewNeedles = [],
      requiredCargoNeedles = [],
      scalarFieldClassifications = [],
      additionalGeneratedPolicies = [],
      requireIdempotence = true,
      requireStrictJson = true,
      requireUnknownFieldZeroization = true,
    } = policy ?? {};

    if (typeof hardeningScript !== "string" || hardeningScript.length === 0) {
      fail("generated proto hardening policy requires a hardeningScript path");
    }
    if (typeof generatedRust !== "string" || generatedRust.length === 0) {
      fail("generated proto hardening policy requires a generatedRust path");
    }
    if (typeof protoSchema !== "string" || protoSchema.length === 0) {
      fail("generated proto hardening policy requires a protoSchema path");
    }
    for (const [policyName, needles] of [
      ["required script", requiredScriptNeedles],
      ["forbidden script", forbiddenScriptNeedles],
      ["required generated", requiredGeneratedNeedles],
      ["forbidden generated", forbiddenGeneratedNeedles],
      ["required view", requiredViewNeedles],
      ["required Cargo", requiredCargoNeedles],
    ]) {
      if (
        !Array.isArray(needles) ||
        needles.some((needle) => typeof needle !== "string")
      ) {
        fail(`generated proto ${policyName} policy must be an array of strings`);
      }
    }
    if (requiredScriptNeedles.length === 0) {
      fail("generated proto hardening policy requires script invariants");
    }
    if (typeof requireIdempotence !== "boolean") {
      fail("generated proto hardening idempotence policy must be a boolean");
    }
    if (requiredGeneratedNeedles.length === 0) {
      fail("generated proto hardening policy requires generated-code invariants");
    }
    if (forbiddenGeneratedNeedles.length === 0) {
      fail("generated proto hardening policy requires forbidden generated-code invariants");
    }
    if (!Array.isArray(scalarFieldClassifications)) {
      fail("generated proto scalar field classifications must be an array");
    }
    if (scalarFieldClassifications.length === 0) {
      fail("generated proto hardening policy requires scalar field classifications");
    }
    const normalizedScalarFieldClassifications = scalarFieldClassifications.map((entry) => {
      if (
        entry !== null &&
        typeof entry === "object" &&
        !Array.isArray(entry) &&
        typeof entry.message === "string" &&
        /^[A-Za-z_][A-Za-z0-9_]*$/u.test(entry.message) &&
        typeof entry.field === "string" &&
        /^[A-Za-z_][A-Za-z0-9_]*$/u.test(entry.field) &&
        (entry.kind === "bytes" || entry.kind === "string") &&
        (entry.sensitivity === "sensitive" || entry.sensitivity === "public") &&
        Object.keys(entry).every((key) =>
          ["message", "field", "kind", "sensitivity"].includes(key),
        )
      ) {
        return {
          field: entry.field,
          kind: entry.kind,
          message: entry.message,
          sensitivity: entry.sensitivity,
        };
      }
      fail(
        "generated proto scalar classifications require { message, field, kind, sensitivity } objects",
      );
    });
    const classificationKeys = new Set();
    for (const entry of normalizedScalarFieldClassifications) {
      const key = `${entry.message}.${entry.field}:${entry.kind}`;
      if (classificationKeys.has(key)) {
        fail(`generated proto scalar classification is duplicated for ${key}`);
      }
      classificationKeys.add(key);
    }
    const protoText = scrubProtoCommentsAndStrings(readText(protoSchema));
    const scalarSchemaKeys = new Set();
    const messagePattern = /^\s*message\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{/gmu;
    for (const messageMatch of protoText.matchAll(messagePattern)) {
      const openIndex = messageMatch.index + messageMatch[0].lastIndexOf("{");
      let depth = 0;
      let closeIndex = -1;
      for (let index = openIndex; index < protoText.length; index += 1) {
        if (protoText[index] === "{") {
          depth += 1;
        } else if (protoText[index] === "}") {
          depth -= 1;
          if (depth === 0) {
            closeIndex = index;
            break;
          }
        }
      }
      if (closeIndex === -1) {
        fail(`${protoSchema} has an unterminated message ${messageMatch[1]}`);
      }
      const body = protoText.slice(openIndex + 1, closeIndex);
      if (/^\s+message\s+[A-Za-z_][A-Za-z0-9_]*\s*\{/mu.test(body)) {
        fail(
          `${protoSchema} nested messages require an explicit scalar-classifier extension`,
        );
      }
      if (/\bmap\s*<[^>]*(?:bytes|string)[^>]*>/u.test(body)) {
        fail(
          `${protoSchema} maps with bytes/string members require an explicit scalar-classifier extension`,
        );
      }
      for (const fieldMatch of body.matchAll(
        /(?:^|[;{}])\s*(?:(?:optional|required|repeated)\s+)?(bytes|string)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\d+(?:\s*\[[^\]]*\])?\s*(?=;)/gmu,
      )) {
        scalarSchemaKeys.add(`${messageMatch[1]}.${fieldMatch[2]}:${fieldMatch[1]}`);
      }
    }
    for (const key of scalarSchemaKeys) {
      if (!classificationKeys.has(key)) {
        fail(`${protoSchema} has unclassified protobuf scalar field ${key}`);
      }
    }
    for (const key of classificationKeys) {
      if (!scalarSchemaKeys.has(key)) {
        fail(`generated proto scalar classification is stale for ${key}`);
      }
    }
    const sensitiveScalarFields = normalizedScalarFieldClassifications.filter(
      (entry) => entry.sensitivity === "sensitive",
    );
    if (sensitiveScalarFields.length === 0) {
      fail("generated proto hardening policy requires at least one sensitive scalar field");
    }
    if (!Array.isArray(additionalGeneratedPolicies)) {
      fail("additional generated hardening policies must be an array");
    }
    for (const [policyName, value] of [
      ["generated view", generatedView],
      ["proto Cargo", protoCargo],
      ["workflow", workflow],
      ["workflow step name", workflowStepName],
      ["workflow step run", workflowStepRun],
    ]) {
      if (value !== undefined && typeof value !== "string") {
        fail(`generated proto ${policyName} policy must be a string`);
      }
    }
    if (
      typeof generatedView === "string" &&
      generatedView.length !== 0 &&
      requiredViewNeedles.length === 0
    ) {
      fail("generated proto hardening policy requires generated view invariants");
    }
    if (
      typeof protoCargo === "string" &&
      protoCargo.length !== 0 &&
      requiredCargoNeedles.length === 0
    ) {
      fail("generated proto hardening policy requires proto Cargo invariants");
    }
    const workflowValues = [workflow, workflowStepName, workflowStepRun];
    const configuredWorkflowValues = workflowValues.filter(
      (value) => typeof value === "string" && value.length !== 0,
    );
    if (
      configuredWorkflowValues.length !== 0 &&
      configuredWorkflowValues.length !== workflowValues.length
    ) {
      fail("generated proto workflow policy must configure path, step name, and run command");
    }

    for (const needle of requiredScriptNeedles) {
      assertContains(hardeningScript, needle);
    }
    if (requireIdempotence) {
      assertContains(hardeningScript, '"--check-idempotent"');
    }
    for (const needle of forbiddenScriptNeedles) {
      assertNotContains(hardeningScript, needle);
    }
    const generatedRustText = readText(generatedRust);
    const messageRegion = (message) => {
      const startNeedle = `pub struct ${message} {`;
      const start = generatedRustText.indexOf(startNeedle);
      if (start === -1) {
        fail(`${generatedRust} does not define generated message ${message}`);
      }
      const remainder = generatedRustText.slice(start + startNeedle.length);
      const nextMessage = /^pub struct [A-Z][A-Za-z0-9]* \{/gmu.exec(remainder);
      const end =
        nextMessage === null
          ? generatedRustText.length
          : start + startNeedle.length + nextMessage.index;
      return generatedRustText.slice(start, end);
    };
    for (const { field, kind, message } of sensitiveScalarFields) {
      const scope = messageRegion(message);
      for (const needle of [
        `.field("${field}", &"<redacted>")`,
        `::zeroize::Zeroize::zeroize(&mut self.${field});`,
        kind === "bytes"
          ? `${field}: ::zeroize::Zeroizing<::buffa::alloc::vec::Vec<u8>>`
          : `${field}: ::zeroize::Zeroizing<::buffa::alloc::string::String>`,
      ]) {
        if (!scope.includes(needle)) {
          fail(`${generatedRust} message ${message} does not contain ${needle}`);
        }
      }
      if (scope.includes(`.field("${field}", &self.${field})`)) {
        fail(`${generatedRust} message ${message} must not expose ${field} in generated Debug output`);
      }
      // Buffa's generated message storage remains Vec<u8>. Sensitive
      // ProtoJSON decoding must stage bytes in a zeroizing temporary, while
      // generated clear and Drop paths wipe the final generated field owner.
    }
    if (requireStrictJson) {
      assertContains(generatedRust, "#[serde(default, deny_unknown_fields)]");
    }
    if (requireUnknownFieldZeroization) {
      for (const needle of [
        "::buffa::UnknownFieldData::LengthDelimited(bytes)",
        "::buffa::UnknownFieldData::Group(fields)",
        "__reallyme_zeroize_unknown_fields(fields);",
      ]) {
        assertContains(hardeningScript, needle);
      }
      assertContains(generatedRust, "fn __reallyme_zeroize_unknown_fields(");
      assertContains(
        generatedRust,
        "::buffa::UnknownFieldData::LengthDelimited(bytes)",
      );
      assertContains(
        generatedRust,
        "::buffa::UnknownFieldData::Group(fields)",
      );
      assertContains(
        generatedRust,
        "__reallyme_zeroize_unknown_fields(fields);",
      );
      assertContains(
        generatedRust,
        "__reallyme_zeroize_unknown_fields(&mut self.__buffa_unknown_fields);",
      );
    }
    for (const needle of requiredGeneratedNeedles) {
      assertContains(generatedRust, needle);
    }
    for (const needle of forbiddenGeneratedNeedles) {
      assertNotContains(generatedRust, needle);
    }
    if (typeof generatedView === "string" && generatedView.length !== 0) {
      for (const needle of requiredViewNeedles) {
        assertContains(generatedView, needle);
      }
    }
    if (typeof protoCargo === "string" && protoCargo.length !== 0) {
      for (const needle of requiredCargoNeedles) {
        assertContains(protoCargo, needle);
      }
    }
    for (const generatedPolicy of additionalGeneratedPolicies) {
      assertTextPolicy({ files: [generatedPolicy] });
    }
    if (
      typeof workflow === "string" &&
      workflow.length !== 0 &&
      typeof workflowStepName === "string" &&
      workflowStepName.length !== 0 &&
      typeof workflowStepRun === "string" &&
      workflowStepRun.length !== 0
    ) {
      assertWorkflowRunStep(workflow, workflowStepName, workflowStepRun);
    }
  };

  const assertReallyMeProtobufReleasePolicy = (policy) => {
    const {
      workflow = ".github/workflows/protobuf-ci.yml",
      corePath = "scripts/release-readiness/core.mjs",
      bufVersion = "1.72.0",
      buffaVersion = "0.9.2",
      installBufStepName = "Install buf",
      installBufUses = null,
      installBufRun = null,
      installBuffaStepName = "Install pinned Buffa generators",
      lintStepName = "Lint protobuf schema",
      generateStepName = "Regenerate protobuf artifacts",
      hardeningPolicy,
      generatedFreshnessMode = false,
      generatedFreshness,
      generatedFreshnessStepName = "Check release readiness generated freshness",
      generatedFreshnessStepRun = "node scripts/check_release_readiness.mjs --generated-freshness",
      workflowMode = "explicit",
    } = policy ?? {};

    assertContains(workflow, `BUFFA_VERSION: ${buffaVersion}`);
    assertContains(workflow, `BUF_VERSION: ${bufVersion}`);
    assertContains(workflow, corePath);
    validateGeneratedArtifactsPolicy(generatedFreshness);

    if (installBufUses !== null) {
      assertWorkflowUsesStep(workflow, installBufStepName, installBufUses);
    }
    if (installBufRun !== null) {
      assertWorkflowRunStep(workflow, installBufStepName, installBufRun);
    }
    assertWorkflowRunStep(
      workflow,
      installBuffaStepName,
      `cargo install protoc-gen-buffa --version "$BUFFA_VERSION" --locked
cargo install protoc-gen-buffa-packaging --version "$BUFFA_VERSION" --locked`,
    );
    assertWorkflowRunStep(workflow, generatedFreshnessStepName, generatedFreshnessStepRun);

    if (workflowMode === "explicit") {
      assertWorkflowRunStep(workflow, lintStepName, "buf lint");
      assertWorkflowRunStep(workflow, generateStepName, "buf generate");
    } else if (workflowMode === "delegated") {
      const duplicateCommands = extractWorkflowSteps(workflow).filter(
        (step) =>
          step.name !== generatedFreshnessStepName &&
          typeof step.run === "string" &&
          /(?:^|\n)\s*buf\s+(?:lint|generate)\b/u.test(step.run),
      );
      if (duplicateCommands.length > 0) {
        fail(
          `${workflow} duplicates protobuf generation outside ${generatedFreshnessStepName}`,
        );
      }
    } else {
      fail(`unsupported protobuf workflow mode ${workflowMode}`);
    }

    assertGeneratedProtoHardeningPolicy(hardeningPolicy);

    if (generatedFreshnessMode) {
      assertGeneratedArtifactsFresh(generatedFreshness);
    }
  };

  const assertReallyMeVendoredCorePolicy = (policy = {}) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("vendored core policy must be an object");
    }
    if (Object.prototype.hasOwnProperty.call(policy, "contractVersion")) {
      fail("numeric release-readiness contractVersion is not supported");
    }
    const {
      scriptPath = "scripts/check_release_readiness.mjs",
      corePath = "scripts/release-readiness/core.mjs",
      version = RELEASE_READINESS_VERSION,
    } = policy;

    if (typeof version !== "string" || !/^\d+\.\d+\.\d+$/u.test(version)) {
      fail("release-readiness version must be an exact semantic version");
    }

    requireTracked(scriptPath);
    requireTracked(corePath);
    const executableCore = scrubJavaScriptCommentsAndStrings(readText(corePath));
    const requireCoreDeclaration = (name) => {
      const declaration = new RegExp(
        `\\b(?:const|function)\\s+${escapeRegExp(name)}\\b`,
        "u",
      );
      if (!declaration.test(executableCore)) {
        fail(`${corePath} must define ${name}`);
      }
    };
    const requireCoreExport = (name) => {
      const exportPattern = new RegExp(`\\b${escapeRegExp(name)}\\s*,`, "u");
      if (!exportPattern.test(executableCore)) {
        fail(`${corePath} must export ${name}`);
      }
    };

    assertContains(corePath, `RELEASE_READINESS_VERSION = "${version}"`);
    for (const name of [
      "assertGeneratedArtifactsFresh",
      "assertGeneratedProtoHardeningPolicy",
      "assertReallyMeProtobufReleasePolicy",
      "assertReallyMeVendoredCorePolicy",
      "assertReallyMeRustProtoRepositoryPolicy",
      "assertCargoMetadataPolicy",
      "assertCargoWorkspacePolicy",
      "assertRepositoryShapePolicy",
      "assertRustSourcePolicy",
      "assertTypeScriptSourcePolicy",
      "assertSwiftSourcePolicy",
      "assertKotlinSourcePolicy",
      "assertTextPolicy",
      "assertSpdxHeaders",
      "assertWorkflowActionsPinned",
      "assertWorkflowPolicy",
      "runCommands",
      "assertProtoContract",
      "assertReallyMeOperationBoundaryContract",
      "assertWorkflowRunStep",
      "assertWorkflowUsesStep",
    ]) {
      requireCoreDeclaration(name);
      requireCoreExport(name);
    }
    for (const identifier of [
      "scalarFieldClassifications",
      "messagePattern",
      "scalarSchemaKeys",
    ]) {
      if (!new RegExp(`\\b${escapeRegExp(identifier)}\\b`, "u").test(executableCore)) {
        fail(`${corePath} must enforce ${identifier}`);
      }
    }
  };

  const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");

  const assertNodeWorkflowJobsPinNode = (workflowOptions = {}) => {
    const workflowDirectoryPath = workflowOptions.workflowDirectory ?? ".github/workflows";
    const workflowDirectory = assertRepositoryDirectory(
      workflowDirectoryPath,
      "workflow directory",
    );
    const nodeVersion = workflowOptions.nodeVersion ?? "24";
    const nodeToolCommands = workflowOptions.nodeToolCommands ?? [
      "node",
      "npm",
      "npx",
      "pnpm",
      "yarn",
      "corepack",
      "bun",
    ];
    if (
      !Array.isArray(nodeToolCommands) ||
      nodeToolCommands.some(
        (command) => typeof command !== "string" || !/^[A-Za-z0-9_-]+$/u.test(command),
      )
    ) {
      fail("Node workflow tool policy must be an array of command names");
    }
    const nodeToolPattern = new RegExp(
      `\\b(?:${nodeToolCommands.map(escapeRegExp).join("|")})\\b`,
      "u",
    );
    for (const workflowFile of readdirSync(workflowDirectory).filter((name) => /\.ya?ml$/u.test(name))) {
      const workflowPath = `${workflowDirectoryPath}/${workflowFile}`;
      const workflow = readText(workflowPath);
      const jobsOffset = workflow.indexOf("\njobs:\n");
      if (jobsOffset === -1) {
        continue;
      }
      const jobs = workflow.slice(jobsOffset + 1);
      const jobHeaders = [...jobs.matchAll(/^  ([a-zA-Z0-9_-]+):\s*$/gm)];
      for (const [index, header] of jobHeaders.entries()) {
        const nextHeader = jobHeaders[index + 1];
        const job = jobs.slice(header.index, nextHeader?.index ?? jobs.length);
        const activeJob = job
          .split("\n")
          .filter((line) => !line.trimStart().startsWith("#"))
          .join("\n");
        if (!nodeToolPattern.test(activeJob)) {
          continue;
        }
        if (!/^\s*uses:\s*actions\/setup-node@[^\s#]+(?:\s+#.*)?$/m.test(activeJob)) {
          fail(`${workflowPath} job ${header[1]} uses Node tooling without actions/setup-node`);
        }
        const pinnedNodeVersion = activeJob.split("\n").some((line) => {
          const match = /^\s*node-version:\s*(.+?)\s*$/u.exec(line);
          return match !== null && unquoteWorkflowScalar(match[1]) === nodeVersion;
        });
        if (!pinnedNodeVersion) {
          fail(`${workflowPath} job ${header[1]} must pin Node ${nodeVersion}`);
        }
      }
    }
  };

  const normalizeWorkflowRunCommand = (command) =>
    command
      .replace(/\r\n/gu, "\n")
      .split("\n")
      .map((line) => line.trimEnd())
      .join("\n")
      .trim();

  const stripWorkflowInlineComment = (value) => {
    let quote = null;
    for (let index = 0; index < value.length; index += 1) {
      const character = value[index];
      if (quote !== null) {
        if (character === quote && value[index - 1] !== "\\") {
          quote = null;
        }
        continue;
      }
      if (character === '"' || character === "'") {
        quote = character;
        continue;
      }
      if (character === "#" && (index === 0 || /\s/u.test(value[index - 1]))) {
        return value.slice(0, index);
      }
    }
    return value;
  };

  const unquoteWorkflowScalar = (value) => {
    const trimmed = stripWorkflowInlineComment(value).trim();
    if (
      (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
      (trimmed.startsWith("'") && trimmed.endsWith("'"))
    ) {
      return trimmed.slice(1, -1);
    }
    return trimmed;
  };

  const countLeadingSpaces = (line) => {
    const match = /^ */u.exec(line);
    return match?.[0].length ?? 0;
  };

  const extractWorkflowStepsFromLines = (path, lines, start, end, jobName = null) => {
    const steps = [];
    for (let index = start; index < end; index += 1) {
      const nameMatch = /^(\s*)-\s+name:\s*(.+?)\s*$/u.exec(lines[index]);
      if (nameMatch === null) {
        continue;
      }

      const stepIndent = nameMatch[1].length;
      const name = unquoteWorkflowScalar(nameMatch[2]);
      let end = lines.length;
      for (let cursor = index + 1; cursor < lines.length; cursor += 1) {
        const candidate = lines[cursor];
        if (countLeadingSpaces(candidate) === stepIndent && /^\s*-\s+/u.test(candidate)) {
          end = cursor;
          break;
        }
      }

      let run = null;
      let uses = null;
      for (let cursor = index + 1; cursor < end; cursor += 1) {
        const runMatch = /^(\s*)run:\s*(.*)\s*$/u.exec(lines[cursor]);
        if (runMatch !== null) {
          if (run !== null) {
            fail(`${path} step ${name} defines run more than once`);
          }
          const runIndent = runMatch[1].length;
          const marker = runMatch[2].trim();
          if (marker === ">") {
            fail(`${path} step ${name} uses an unsupported folded run scalar`);
          }
          if (marker === "|") {
            const blockLines = [];
            for (let blockCursor = cursor + 1; blockCursor < end; blockCursor += 1) {
              const blockLine = lines[blockCursor];
              if (blockLine.trim().length !== 0 && countLeadingSpaces(blockLine) <= runIndent) {
                break;
              }
              blockLines.push(blockLine);
            }
            const nonBlankIndents = blockLines
              .filter((line) => line.trim().length !== 0)
              .map((line) => countLeadingSpaces(line));
            const blockIndent =
              nonBlankIndents.length === 0 ? runIndent + 2 : Math.min(...nonBlankIndents);
            run = blockLines.map((line) => line.slice(Math.min(blockIndent, line.length))).join("\n");
          } else {
            run = unquoteWorkflowScalar(marker);
          }
        }

        const usesMatch = /^\s*uses:\s*(.+?)\s*$/u.exec(lines[cursor]);
        if (usesMatch !== null) {
          if (uses !== null) {
            fail(`${path} step ${name} defines uses more than once`);
          }
          uses = unquoteWorkflowScalar(usesMatch[1]);
        }
      }

      steps.push({ job: jobName, name, run, uses });
    }
    return steps;
  };

  const extractWorkflowJobs = (path) => {
    const lines = readText(path).replace(/\r\n/gu, "\n").split("\n");
    const jobsLine = lines.findIndex((line) => /^jobs:\s*$/u.test(line));
    if (jobsLine === -1) {
      return [];
    }
    const headers = [];
    for (let index = jobsLine + 1; index < lines.length; index += 1) {
      const match = /^  ([A-Za-z0-9_-]+):\s*$/u.exec(lines[index]);
      if (match !== null) {
        headers.push({ name: match[1], index });
      }
    }
    return headers.map((header, index) => ({
      name: header.name,
      start: header.index,
      end: headers[index + 1]?.index ?? lines.length,
      lines,
    }));
  };

  const parseWorkflowPermissionBlock = (path, lines, headerIndex, headerIndent, label) => {
    const permissions = new Map();
    for (let index = headerIndex + 1; index < lines.length; index += 1) {
      const line = lines[index];
      const trimmed = line.trim();
      if (trimmed.length === 0 || trimmed.startsWith("#")) {
        continue;
      }
      const indent = countLeadingSpaces(line);
      if (indent <= headerIndent) {
        break;
      }
      const match = /^\s*([A-Za-z0-9_-]+):\s*(read|write|none)\s*(?:#.*)?$/u.exec(line);
      if (indent !== headerIndent + 2 || match === null) {
        fail(`${path} ${label} permissions must be a flat explicit mapping`);
      }
      if (permissions.has(match[1])) {
        fail(`${path} ${label} permission ${match[1]} is defined more than once`);
      }
      permissions.set(match[1], match[2]);
    }
    if (permissions.size === 0) {
      fail(`${path} ${label} permissions mapping must not be empty`);
    }
    return permissions;
  };

  const validateExpectedPermissions = (path, label, value) => {
    if (
      value === null ||
      typeof value !== "object" ||
      Array.isArray(value) ||
      Object.entries(value).some(
        ([scope, access]) =>
          !/^[A-Za-z0-9_-]+$/u.test(scope) ||
          !["read", "write", "none"].includes(access),
      )
    ) {
      fail(`${path} ${label} expected permissions must be an explicit mapping`);
    }
    return new Map(Object.entries(value));
  };

  const assertPermissionMapsEqual = (path, label, actual, expected) => {
    if (
      actual.size !== expected.size ||
      [...expected].some(([scope, access]) => actual.get(scope) !== access)
    ) {
      fail(`${path} ${label} permissions changed`);
    }
  };

  const assertWorkflowPermissionsPolicy = (policy) => {
    const { path, workflow, jobs = {} } = policy ?? {};
    if (typeof path !== "string" || path.length === 0) {
      fail("workflow permissions policy requires a path");
    }
    const expectedWorkflow = validateExpectedPermissions(path, "workflow", workflow);
    if (jobs === null || typeof jobs !== "object" || Array.isArray(jobs)) {
      fail(`${path} expected job permissions must be an explicit mapping`);
    }

    const lines = readText(path).replace(/\r\n/gu, "\n").split("\n");
    const workflowHeaders = lines
      .map((line, index) => ({ index, matches: /^permissions:\s*$/u.test(line) }))
      .filter((entry) => entry.matches);
    if (workflowHeaders.length !== 1) {
      fail(`${path} must define exactly one top-level permissions mapping`);
    }
    const actualWorkflow = parseWorkflowPermissionBlock(
      path,
      lines,
      workflowHeaders[0].index,
      0,
      "workflow",
    );
    assertPermissionMapsEqual(path, "workflow", actualWorkflow, expectedWorkflow);

    const actualJobs = new Map();
    for (const job of extractWorkflowJobs(path)) {
      const headers = [];
      for (let index = job.start + 1; index < job.end; index += 1) {
        if (/^ {4}permissions:\s+\S/u.test(lines[index])) {
          fail(`${path} job ${job.name} permissions must be a flat explicit mapping`);
        }
        if (/^ {4}permissions:\s*$/u.test(lines[index])) {
          headers.push(index);
        }
      }
      if (headers.length > 1) {
        fail(`${path} job ${job.name} defines permissions more than once`);
      }
      if (headers.length === 1) {
        actualJobs.set(
          job.name,
          parseWorkflowPermissionBlock(path, lines, headers[0], 4, `job ${job.name}`),
        );
      }
    }

    const expectedJobs = new Map();
    for (const [jobName, permissions] of Object.entries(jobs)) {
      if (!/^[A-Za-z0-9_-]+$/u.test(jobName)) {
        fail(`${path} expected job permission name is invalid`);
      }
      expectedJobs.set(
        jobName,
        validateExpectedPermissions(path, `job ${jobName}`, permissions),
      );
    }
    if (
      actualJobs.size !== expectedJobs.size ||
      [...actualJobs.keys()].some((jobName) => !expectedJobs.has(jobName))
    ) {
      fail(`${path} jobs with explicit permissions changed`);
    }
    for (const [jobName, expected] of expectedJobs) {
      const actual = actualJobs.get(jobName);
      if (actual === undefined) {
        fail(`${path} job ${jobName} is missing explicit permissions`);
      }
      assertPermissionMapsEqual(path, `job ${jobName}`, actual, expected);
    }
  };

  const extractWorkflowSteps = (path) => {
    const jobs = extractWorkflowJobs(path);
    if (jobs.length === 0) {
      const lines = readText(path).replace(/\r\n/gu, "\n").split("\n");
      return extractWorkflowStepsFromLines(path, lines, 0, lines.length);
    }
    return jobs.flatMap((job) =>
      extractWorkflowStepsFromLines(path, job.lines, job.start, job.end, job.name),
    );
  };

  const findWorkflowStep = (path, stepName, jobName = null) => {
    if (typeof stepName !== "string" || stepName.length === 0) {
      fail(`${path} workflow step policy requires a step name`);
    }
    if (jobName !== null && (typeof jobName !== "string" || jobName.length === 0)) {
      fail(`${path} workflow step ${stepName} job must be null or a non-empty string`);
    }
    const steps = extractWorkflowSteps(path).filter(
      (candidate) =>
        candidate.name === stepName && (jobName === null || candidate.job === jobName),
    );
    const location = jobName === null ? stepName : `${jobName}/${stepName}`;
    if (steps.length === 0) {
      fail(`${path} is missing workflow step ${location}`);
    }
    if (steps.length > 1) {
      fail(`${path} defines workflow step ${location} more than once`);
    }
    return steps[0];
  };

  const assertWorkflowRunStep = (path, stepName, expectedRun, jobName = null) => {
    if (typeof expectedRun !== "string" || expectedRun.length === 0) {
      fail(`${path} step ${stepName} requires an expected run command`);
    }
    const step = findWorkflowStep(path, stepName, jobName);
    if (step.run === null) {
      fail(`${path} step ${stepName} does not define a run command`);
    }
    const actual = normalizeWorkflowRunCommand(step.run);
    const expected = normalizeWorkflowRunCommand(expectedRun);
    if (actual !== expected) {
      fail(`${path} step ${stepName} run command changed`);
    }
  };

  const assertWorkflowUsesStep = (path, stepName, expectedUses, jobName = null) => {
    if (typeof expectedUses !== "string" || expectedUses.length === 0) {
      fail(`${path} step ${stepName} requires an expected action`);
    }
    const step = findWorkflowStep(path, stepName, jobName);
    const expected = unquoteWorkflowScalar(expectedUses);
    if (step.uses !== expected) {
      fail(`${path} step ${stepName} must use ${expected}`);
    }
  };

  const assertWorkflowPolicy = (policy) => {
    const {
      path,
      required = [],
      forbidden = [],
      runSteps = [],
      usesSteps = [],
    } = policy ?? {};
    if (typeof path !== "string" || path.length === 0) {
      fail("workflow policy requires a path");
    }
    for (const [policyName, values] of [
      ["required", required],
      ["forbidden", forbidden],
    ]) {
      if (
        !Array.isArray(values) ||
        values.some((value) => typeof value !== "string")
      ) {
        fail(`${path} workflow ${policyName} policy must be an array of strings`);
      }
    }
    if (!Array.isArray(runSteps) || !Array.isArray(usesSteps)) {
      fail(`${path} workflow step policies must be arrays`);
    }
    for (const needle of required) {
      assertContains(path, needle);
    }
    for (const needle of forbidden) {
      assertNotContains(path, needle);
    }
    for (const step of runSteps) {
      assertWorkflowRunStep(path, step?.name, step?.run, step?.job ?? null);
    }
    for (const step of usesSteps) {
      assertWorkflowUsesStep(path, step?.name, step?.uses, step?.job ?? null);
    }
  };

  const assertWorkflowActionsPinned = (workflowOptions = {}) => {
    const workflowDirectoryPath = workflowOptions.workflowDirectory ?? ".github/workflows";
    const workflowDirectory = assertRepositoryDirectory(
      workflowDirectoryPath,
      "workflow directory",
    );
    const allowedNonShaUsesPolicy = workflowOptions.allowedNonShaUses ?? [];
    if (
      !Array.isArray(allowedNonShaUsesPolicy) ||
      allowedNonShaUsesPolicy.some((uses) => typeof uses !== "string")
    ) {
      fail("allowed non-SHA workflow actions must be an array of strings");
    }
    const allowedNonShaUses = new Set(allowedNonShaUsesPolicy);
    const allowLocalActions = workflowOptions.allowLocalActions ?? true;
    const allowDockerActions = workflowOptions.allowDockerActions ?? false;
    if (typeof allowLocalActions !== "boolean" || typeof allowDockerActions !== "boolean") {
      fail("workflow action allow policies must be booleans");
    }
    const fullCommitUse =
      /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:\/[A-Za-z0-9_./-]+)?@[0-9a-f]{40}$/u;

    for (const workflowFile of readdirSync(workflowDirectory).filter((name) => /\.ya?ml$/u.test(name))) {
      const workflowPath = `${workflowDirectoryPath}/${workflowFile}`;
      const lines = readText(workflowPath).replace(/\r\n/gu, "\n").split("\n");
      for (const [index, line] of lines.entries()) {
        const match = /^\s*(?:-\s+)?uses:\s*(.+?)\s*$/u.exec(line);
        if (match === null) {
          continue;
        }
        const uses = unquoteWorkflowScalar(match[1]);
        if (
          allowedNonShaUses.has(uses)
        ) {
          continue;
        }
        if (uses.startsWith("./")) {
          if (!allowLocalActions) {
            fail(`${workflowPath}:${index + 1} local action ${uses} is not allowed`);
          }
          resolveRepositoryPath(uses.slice(2), "local workflow action");
          continue;
        }
        if (uses.startsWith("docker://")) {
          if (!allowDockerActions) {
            fail(`${workflowPath}:${index + 1} Docker action ${uses} is not allowed`);
          }
          if (!/^docker:\/\/[^@\s]+@sha256:[0-9a-f]{64}$/u.test(uses)) {
            fail(
              `${workflowPath}:${index + 1} Docker action ${uses} is not pinned to a sha256 digest`,
            );
          }
          continue;
        }
        if (!fullCommitUse.test(uses)) {
          fail(`${workflowPath}:${index + 1} action ${uses} is not pinned to a full commit SHA`);
        }
      }
    }
  };

  const extractWorkflowRunCommands = (path) => {
    const lines = readText(path).replace(/\r\n/gu, "\n").split("\n");
    const commands = [];
    for (let index = 0; index < lines.length; index += 1) {
      const runMatch = /^(\s*)(?:-\s+)?run:\s*(.*)\s*$/u.exec(lines[index]);
      if (runMatch === null) {
        continue;
      }
      const runIndent = runMatch[1].length;
      const marker = runMatch[2].trim();
      if (marker === ">") {
        fail(`${path} uses an unsupported folded run scalar`);
      }
      if (marker === "|") {
        const blockLines = [];
        for (let blockCursor = index + 1; blockCursor < lines.length; blockCursor += 1) {
          const blockLine = lines[blockCursor];
          if (blockLine.trim().length !== 0 && countLeadingSpaces(blockLine) <= runIndent) {
            break;
          }
          blockLines.push(blockLine);
        }
        const nonBlankIndents = blockLines
          .filter((line) => line.trim().length !== 0)
          .map((line) => countLeadingSpaces(line));
        const blockIndent =
          nonBlankIndents.length === 0 ? runIndent + 2 : Math.min(...nonBlankIndents);
        commands.push(
          blockLines.map((line) => line.slice(Math.min(blockIndent, line.length))).join("\n"),
        );
      } else {
        commands.push(unquoteWorkflowScalar(marker));
      }
    }
    return commands;
  };

  const assertCargoFuzzWorkflowPolicy = (policy) => {
    const {
      workflow = ".github/workflows/fuzz.yml",
      version,
      gitSource,
      minimumInstallations = 2,
      requiredInstallSteps = [],
    } = policy ?? {};
    if (typeof workflow !== "string" || workflow.length === 0) {
      fail("cargo-fuzz workflow policy requires a workflow path");
    }
    const configuredSources = [
      Object.prototype.hasOwnProperty.call(policy ?? {}, "version"),
      Object.prototype.hasOwnProperty.call(policy ?? {}, "gitSource"),
    ].filter(Boolean).length;
    if (configuredSources !== 1) {
      fail("cargo-fuzz workflow policy requires exactly one exact version or Git revision");
    }
    const hasVersion = Object.prototype.hasOwnProperty.call(policy ?? {}, "version");
    if (hasVersion && (typeof version !== "string" || !/^\d+\.\d+\.\d+$/u.test(version))) {
      fail("cargo-fuzz workflow policy requires an exact semantic version");
    }
    const hasGitSource = Object.prototype.hasOwnProperty.call(policy ?? {}, "gitSource");
    if (
      hasGitSource &&
      (gitSource === null ||
        typeof gitSource !== "object" ||
        Array.isArray(gitSource) ||
        typeof gitSource.url !== "string" ||
        !/^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.git$/u.test(gitSource.url) ||
        typeof gitSource.revision !== "string" ||
        !/^[0-9a-f]{40}$/u.test(gitSource.revision))
    ) {
      fail("cargo-fuzz workflow policy requires an exact GitHub repository URL and revision");
    }
    if (!Number.isSafeInteger(minimumInstallations) || minimumInstallations < 1) {
      fail("cargo-fuzz workflow policy requires a positive installation count");
    }
    if (
      !Array.isArray(requiredInstallSteps) ||
      requiredInstallSteps.some(
        (step) =>
          step === null ||
          typeof step !== "object" ||
          Array.isArray(step) ||
          typeof step.name !== "string" ||
          step.name.length === 0 ||
          (step.job !== undefined &&
            (typeof step.job !== "string" || step.job.length === 0)),
      )
    ) {
      fail("cargo-fuzz required install steps must be named workflow steps");
    }

    const text = readText(workflow).replace(/\r\n/gu, "\n");
    const workflowRuns = extractWorkflowSteps(workflow)
      .filter((step) => step.run !== null)
      .map((step) => ({ ...step, command: normalizeWorkflowRunCommand(step.run) }));
    const allInstallCommands = extractWorkflowRunCommands(workflow).filter(
      (command) =>
        /^cargo\s+install(?:\s|$)/mu.test(normalizeWorkflowRunCommand(command)) &&
        /(?:^|\s)cargo-fuzz(?:\s|$)/u.test(normalizeWorkflowRunCommand(command)),
    );
    const installSteps = workflowRuns.filter((step) =>
      /^cargo\s+install(?:\s|$)/mu.test(step.command) &&
      /(?:^|\s)cargo-fuzz(?:\s|$)/u.test(step.command),
    );
    if (installSteps.length !== allInstallCommands.length) {
      fail(`${workflow} cargo-fuzz installation must be in a named workflow step`);
    }
    if (installSteps.length < minimumInstallations) {
      fail(
        `${workflow} must install cargo-fuzz at least ${minimumInstallations} times`,
      );
    }
    const environmentVersion = hasVersion
      ? new RegExp(
          `^\\s*CARGO_FUZZ_VERSION:\\s*["']?${version.replaceAll(".", "\\.")}["']?\\s*$`,
          "mu",
        )
      : null;
    for (const expected of requiredInstallSteps) {
      const matches = installSteps.filter(
        (step) =>
          step.name === expected.name &&
          (expected.job === undefined || step.job === expected.job),
      );
      if (matches.length === 0) {
        const location =
          expected.job === undefined
            ? expected.name
            : `${expected.job}/${expected.name}`;
        fail(`${workflow} is missing cargo-fuzz install step ${location}`);
      }
      if (matches.length > 1) {
        const location =
          expected.job === undefined
            ? expected.name
            : `${expected.job}/${expected.name}`;
        fail(`${workflow} defines cargo-fuzz install step ${location} more than once`);
      }
    }
    for (const step of installSteps) {
      const command = normalizeWorkflowRunCommand(step.run);
      if (!/(?:^|\s)--locked(?:\s|$)/u.test(command)) {
        fail(`${workflow} cargo-fuzz installation must use --locked`);
      }
      if (hasGitSource) {
        const expected =
          `cargo install --git ${gitSource.url} --rev ${gitSource.revision} --locked cargo-fuzz`;
        if (command !== expected) {
          fail(`${workflow} cargo-fuzz installation must use the configured exact Git revision`);
        }
        continue;
      }
      const usesLiteralVersion = new RegExp(
        `(?:^|\\s)--version\\s+${version.replaceAll(".", "\\.")}(?:\\s|$)`,
        "u",
      ).test(command);
      const usesEnvironmentVersion =
        command.includes('--version "$CARGO_FUZZ_VERSION"') ||
        command.includes("--version '$CARGO_FUZZ_VERSION'") ||
        command.includes("--version $CARGO_FUZZ_VERSION");
      if (
        !usesLiteralVersion &&
        !(usesEnvironmentVersion && environmentVersion !== null && environmentVersion.test(text))
      ) {
        fail(
          `${workflow} cargo-fuzz installation must pin version ${version}`,
        );
      }
    }
  };

  const assertReallyMeRustProtoRepositoryPolicy = (policy) => {
    if (policy === null || typeof policy !== "object" || Array.isArray(policy)) {
      fail("ReallyMe Rust protobuf repository policy must be an object");
    }
    const {
      generatedFreshnessMode,
      vendoredCore = {},
      workflowActions = {},
      nodeWorkflows = {},
      cargoFuzz,
      cargoWorkspace = {},
      repositoryShape,
      rustSource,
      typescriptSource,
      swiftSource,
      kotlinSource,
      spdx = {},
      protobufBoundary,
      granularProviderBoundary,
      protobufRelease,
      cargoMetadata,
      text,
      workflows = [],
      retiredPaths = [],
    } = policy;
    if (typeof generatedFreshnessMode !== "boolean") {
      fail("ReallyMe Rust protobuf repository policy requires generatedFreshnessMode");
    }
    for (const [name, value] of [
      ["vendoredCore", vendoredCore],
      ["workflowActions", workflowActions],
      ["nodeWorkflows", nodeWorkflows],
      ["cargoFuzz", cargoFuzz],
      ["cargoWorkspace", cargoWorkspace],
      ["spdx", spdx],
      ["protobufBoundary", protobufBoundary],
      ["protobufRelease", protobufRelease],
    ]) {
      if (value === null || typeof value !== "object" || Array.isArray(value)) {
        fail(`ReallyMe Rust protobuf repository policy ${name} must be an object`);
      }
    }
    if (
      rustSource !== undefined &&
      (rustSource === null || typeof rustSource !== "object" || Array.isArray(rustSource))
    ) {
      fail("ReallyMe Rust protobuf repository policy rustSource must be an object");
    }
    if (
      repositoryShape !== undefined &&
      (repositoryShape === null ||
        typeof repositoryShape !== "object" ||
        Array.isArray(repositoryShape))
    ) {
      fail("ReallyMe Rust protobuf repository policy repositoryShape must be an object");
    }
    for (const [name, value] of [
      ["typescriptSource", typescriptSource],
      ["swiftSource", swiftSource],
      ["kotlinSource", kotlinSource],
    ]) {
      if (
        value !== undefined &&
        (value === null || typeof value !== "object" || Array.isArray(value))
      ) {
        fail(`ReallyMe Rust protobuf repository policy ${name} must be an object`);
      }
    }
    if (
      cargoMetadata !== undefined &&
      (cargoMetadata === null ||
        typeof cargoMetadata !== "object" ||
        Array.isArray(cargoMetadata))
    ) {
      fail("ReallyMe Rust protobuf repository policy cargoMetadata must be an object");
    }
    if (
      granularProviderBoundary !== undefined &&
      (granularProviderBoundary === null ||
        typeof granularProviderBoundary !== "object" ||
        Array.isArray(granularProviderBoundary))
    ) {
      fail("ReallyMe Rust protobuf repository policy granularProviderBoundary must be an object");
    }
    if (
      text !== undefined &&
      (text === null || typeof text !== "object" || Array.isArray(text))
    ) {
      fail("ReallyMe Rust protobuf repository policy text must be an object");
    }
    if (
      !Array.isArray(workflows) ||
      workflows.some(
        (workflow) =>
          workflow === null ||
          typeof workflow !== "object" ||
          Array.isArray(workflow),
      )
    ) {
      fail("ReallyMe Rust protobuf repository workflows must be an array of objects");
    }
    if (
      Object.prototype.hasOwnProperty.call(
        protobufRelease,
        "generatedFreshnessMode",
      )
    ) {
      fail(
        "generatedFreshnessMode must be configured once at the repository-policy level",
      );
    }

    assertReallyMeVendoredCorePolicy(vendoredCore);
    assertWorkflowActionsPinned(workflowActions);
    assertNodeWorkflowJobsPinNode(nodeWorkflows);
    assertCargoFuzzWorkflowPolicy(cargoFuzz);
    assertCargoWorkspacePolicy(cargoWorkspace);
    if (repositoryShape !== undefined) {
      assertRepositoryShapePolicy(repositoryShape);
    }
    if (rustSource !== undefined) {
      assertRustSourcePolicy(rustSource);
    }
    if (typescriptSource !== undefined) {
      assertTypeScriptSourcePolicy(typescriptSource);
    }
    if (swiftSource !== undefined) {
      assertSwiftSourcePolicy(swiftSource);
    }
    if (kotlinSource !== undefined) {
      assertKotlinSourcePolicy(kotlinSource);
    }
    assertSpdxHeaders(spdx);
    assertReallyMeOperationBoundaryContract(protobufBoundary);
    if (granularProviderBoundary !== undefined) {
      assertGranularProviderBoundary(granularProviderBoundary);
    }
    assertPathsAbsent(retiredPaths);
    assertReallyMeProtobufReleasePolicy({
      ...protobufRelease,
      generatedFreshnessMode,
    });
    if (cargoMetadata !== undefined) {
      assertCargoMetadataPolicy(cargoMetadata);
    }
    if (text !== undefined) {
      assertTextPolicy(text);
    }
    for (const workflow of workflows) {
      assertWorkflowPolicy(workflow);
    }
  };

  const stripProtoLineComments = (text) =>
    text
      .split("\n")
      .map((line) => {
        const commentStart = line.indexOf("//");
        return commentStart === -1 ? line : line.slice(0, commentStart);
      })
      .join("\n");

  const extractProtoBlocks = (protoText, keyword) => {
    const blocks = [];
    const declarationPattern = new RegExp(`\\b${keyword}\\s+([A-Za-z_][A-Za-z0-9_]*)\\s*\\{`, "g");
    let match = declarationPattern.exec(protoText);
    while (match !== null) {
      let depth = 1;
      let cursor = declarationPattern.lastIndex;
      while (cursor < protoText.length && depth > 0) {
        const char = protoText[cursor];
        if (char === "{") {
          depth += 1;
        } else if (char === "}") {
          depth -= 1;
        }
        cursor += 1;
      }
      if (depth !== 0) {
        fail(`proto ${keyword} ${match[1]} has unbalanced braces`);
      }
      blocks.push({
        name: match[1],
        body: protoText.slice(declarationPattern.lastIndex, cursor - 1),
      });
      declarationPattern.lastIndex = cursor;
      match = declarationPattern.exec(protoText);
    }
    return blocks;
  };

  const parseProtoReservations = (path, ownerKind, ownerName, body) => {
    const numberRanges = [];
    const names = new Set();
    for (const declaration of body.matchAll(/\breserved\s+([^;]+);/gu)) {
      for (const rawEntry of declaration[1].split(",")) {
        const entry = rawEntry.trim();
        const nameMatch = /^"([A-Za-z_][A-Za-z0-9_]*)"$/u.exec(entry);
        if (nameMatch !== null) {
          if (names.has(nameMatch[1])) {
            fail(`${path} ${ownerKind} ${ownerName} reserves name ${nameMatch[1]} more than once`);
          }
          names.add(nameMatch[1]);
          continue;
        }
        const rangeMatch = /^(-?\d+)(?:\s+to\s+(-?\d+|max))?$/u.exec(entry);
        if (rangeMatch === null) {
          fail(`${path} ${ownerKind} ${ownerName} has unsupported reservation ${entry}`);
        }
        const start = Number.parseInt(rangeMatch[1], 10);
        const end =
          rangeMatch[2] === undefined
            ? start
            : rangeMatch[2] === "max"
              ? ownerKind === "message"
                ? 536_870_911
                : 2_147_483_647
              : Number.parseInt(rangeMatch[2], 10);
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start > end) {
          fail(`${path} ${ownerKind} ${ownerName} has invalid reservation ${entry}`);
        }
        if (
          numberRanges.some(
            ([existingStart, existingEnd]) =>
              start <= existingEnd && end >= existingStart,
          )
        ) {
          fail(`${path} ${ownerKind} ${ownerName} has overlapping reserved number ranges`);
        }
        numberRanges.push([start, end]);
      }
    }
    return { numberRanges, names };
  };

  const isReservedProtoNumber = (number, reservations) =>
    reservations.numberRanges.some(([start, end]) => number >= start && number <= end);

  const assertProtoContract = (path) => {
    const proto = stripProtoLineComments(readText(path));

    for (const block of extractProtoBlocks(proto, "enum")) {
      const reservations = parseProtoReservations(path, "enum", block.name, block.body);
      const names = new Set();
      const numbers = new Set();
      const values = [
        ...block.body.matchAll(
          /^\s*([A-Z][A-Z0-9_]*)\s*=\s*(-?\d+)\s*(?:\[[^\]]*\])?\s*;/gmu,
        ),
      ].map((match) => ({
        name: match[1],
        number: Number.parseInt(match[2], 10),
      }));
      if (values.length === 0) {
        fail(`${path} enum ${block.name} must define at least one value`);
      }
      if (values[0].number !== 0 || !values[0].name.endsWith("_UNSPECIFIED")) {
        fail(`${path} enum ${block.name} must start with an UNSPECIFIED value at zero`);
      }
      for (const value of values) {
        if (
          !Number.isSafeInteger(value.number) ||
          value.number < -2_147_483_648 ||
          value.number > 2_147_483_647
        ) {
          fail(`${path} enum ${block.name} value ${value.name} is outside int32 range`);
        }
        if (names.has(value.name)) {
          fail(`${path} enum ${block.name} defines name ${value.name} more than once`);
        }
        if (numbers.has(value.number)) {
          fail(`${path} enum ${block.name} reuses number ${value.number}`);
        }
        if (reservations.names.has(value.name)) {
          fail(`${path} enum ${block.name} reuses reserved name ${value.name}`);
        }
        if (isReservedProtoNumber(value.number, reservations)) {
          fail(`${path} enum ${block.name} reuses reserved number ${value.number}`);
        }
        names.add(value.name);
        numbers.add(value.number);
      }
    }

    for (const block of extractProtoBlocks(proto, "message")) {
      const reservations = parseProtoReservations(path, "message", block.name, block.body);
      const names = new Set();
      const numbers = new Set();
      const fields = [
        ...block.body.matchAll(
          /^\s*(?:optional\s+|repeated\s+)?(?:map\s*<[^>]+>|[A-Za-z_][A-Za-z0-9_.]*)\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)\s*(?:\[[^\]]*\])?\s*;/gmu,
        ),
      ].map((match) => ({
        name: match[1],
        number: Number.parseInt(match[2], 10),
      }));
      for (const field of fields) {
        if (
          !Number.isSafeInteger(field.number) ||
          field.number < 1 ||
          field.number > 536_870_911 ||
          (field.number >= 19_000 && field.number <= 19_999)
        ) {
          fail(`${path} message ${block.name} field ${field.name} has invalid number ${field.number}`);
        }
        if (names.has(field.name)) {
          fail(`${path} message ${block.name} defines field ${field.name} more than once`);
        }
        if (numbers.has(field.number)) {
          fail(`${path} message ${block.name} reuses field number ${field.number}`);
        }
        if (reservations.names.has(field.name)) {
          fail(`${path} message ${block.name} reuses reserved name ${field.name}`);
        }
        if (isReservedProtoNumber(field.number, reservations)) {
          fail(`${path} message ${block.name} reuses reserved field number ${field.number}`);
        }
        names.add(field.name);
        numbers.add(field.number);
      }
    }
  };

  const assertReallyMeOperationBoundaryContract = (policy) => {
    const {
      protoPath,
      operationRequest,
      operationResponse,
      operationResult = "CodecOperationResult",
      errorMessage = "CodecError",
      protoReadme,
      protoCargo,
      wirePath,
      codecPath = wirePath,
      bufGen = "buf.gen.yaml",
      processOperationNeedle = "pub fn process_operation_response(",
      processOperationJsonNeedle = "pub fn process_operation_response_json(",
      binaryResponseNeedle = "CodecOperationResponse",
      requiredCodecNeedles = [],
      forbiddenCodecNeedles = [],
      sdkAdapters = [],
      allowServices = true,
    } = policy ?? {};
    for (const [name, value] of Object.entries({
      protoPath,
      protoReadme,
      protoCargo,
      wirePath,
      codecPath,
    })) {
      if (typeof value !== "string" || value.length === 0) {
        fail(`operation boundary policy ${name} must be a non-empty string`);
      }
    }
    if (
      !Array.isArray(requiredCodecNeedles) ||
      requiredCodecNeedles.some((needle) => typeof needle !== "string" || needle.length === 0) ||
      !Array.isArray(forbiddenCodecNeedles) ||
      forbiddenCodecNeedles.some((needle) => typeof needle !== "string" || needle.length === 0)
    ) {
      fail("operation boundary codec needles must be arrays of non-empty strings");
    }
    if (
      !Array.isArray(sdkAdapters) ||
      sdkAdapters.some(
        (adapter) =>
          adapter === null ||
          typeof adapter !== "object" ||
          Array.isArray(adapter),
      )
    ) {
      fail("operation boundary policy sdkAdapters must be an array of objects");
    }
    if (typeof allowServices !== "boolean") {
      fail("operation boundary policy allowServices must be a boolean");
    }
    for (const [name, value] of Object.entries({
      operationRequest,
      operationResponse,
      operationResult,
      errorMessage,
    })) {
      if (
        typeof value !== "string" ||
        !/^[A-Za-z_][A-Za-z0-9_]*$/u.test(value)
      ) {
        fail(`operation boundary policy ${name} must be a protobuf identifier`);
      }
    }

    assertProtoContract(protoPath);
    const proto = stripProtoLineComments(readText(protoPath));
    if (!allowServices && extractProtoBlocks(proto, "service").length !== 0) {
      fail(`${protoPath} must define messages only and no protobuf service`);
    }
    const operationBlock = extractProtoBlocks(proto, "message").find(
      (block) => block.name === operationRequest,
    );
    if (operationBlock === undefined || !/\boneof\s+operation\s*\{/u.test(operationBlock.body)) {
      fail(`${protoPath} ${operationRequest} must define oneof operation`);
    }
    const responseBlock = extractProtoBlocks(proto, "message").find(
      (block) => block.name === operationResponse,
    );
    if (
      responseBlock === undefined ||
      !/\boneof\s+outcome\s*\{/u.test(responseBlock.body) ||
      !new RegExp(`^\\s*${operationResult}\\s+result\\s*=\\s*1\\s*;`, "mu").test(
        responseBlock.body,
      ) ||
      !new RegExp(`^\\s*${errorMessage}\\s+error\\s*=\\s*2\\s*;`, "mu").test(
        responseBlock.body,
      )
    ) {
      fail(`${protoPath} ${operationResponse} must contain a generated result/error outcome oneof`);
    }
    const resultBlock = extractProtoBlocks(proto, "message").find(
      (block) => block.name === operationResult,
    );
    if (resultBlock === undefined || !/\boneof\s+result\s*\{/u.test(resultBlock.body)) {
      fail(`${protoPath} ${operationResult} must define oneof result`);
    }

    assertContains(
      protoReadme,
      "JSON is a generated ProtoJSON request convenience. Results remain one fully",
    );
    assertContains(bufGen, "local: protoc-gen-buffa");
    assertContains(bufGen, "views=true");
    assertContains(bufGen, "json=true");
    assertContains(protoCargo, '"buffa/json"');
    assertContains(protoCargo, "zeroize");
    assertContains(wirePath, operationRequest);
    assertContains(wirePath, operationResponse);
    assertContains(wirePath, "Zeroizing<Vec<u8>>");
    assertContains(wirePath, processOperationNeedle);
    assertContains(wirePath, processOperationJsonNeedle);
    assertContains(codecPath, "DecodeOptions::new()");
    assertContains(codecPath, binaryResponseNeedle);
    for (const needle of requiredCodecNeedles) {
      assertContains(codecPath, needle);
    }
    for (const needle of forbiddenCodecNeedles) {
      assertNotContains(codecPath, needle);
    }
    assertNotContains(wirePath, "pub fn process_json(");
    assertNotContains(wirePath, "pub fn process_proto_with_operation");
    assertNotContains(wirePath, "pub fn process_proto_operation");
    assertNotContains(wirePath, "CodecProtoResultEnvelope");

    for (const [index, adapter] of sdkAdapters.entries()) {
      const {
        path,
        processOperationNeedle: adapterProcessOperationNeedle,
        processOperationJsonNeedle: adapterProcessOperationJsonNeedle,
        binaryResponseNeedle: adapterBinaryResponseNeedle = operationResponse,
        requiredNeedles = [],
        forbiddenNeedles = [],
      } = adapter;
      for (const [name, value] of Object.entries({
        path,
        processOperationNeedle: adapterProcessOperationNeedle,
        processOperationJsonNeedle: adapterProcessOperationJsonNeedle,
        binaryResponseNeedle: adapterBinaryResponseNeedle,
      })) {
        if (typeof value !== "string" || value.length === 0) {
          fail(
            `operation boundary sdkAdapters[${index}].${name} must be a non-empty string`,
          );
        }
      }
      for (const [name, needles] of Object.entries({
        requiredNeedles,
        forbiddenNeedles,
      })) {
        if (
          !Array.isArray(needles) ||
          needles.some(
            (needle) => typeof needle !== "string" || needle.length === 0,
          )
        ) {
          fail(
            `operation boundary sdkAdapters[${index}].${name} must be an array of non-empty strings`,
          );
        }
      }
      assertContains(path, adapterProcessOperationNeedle);
      assertContains(path, adapterProcessOperationJsonNeedle);
      assertContains(path, adapterBinaryResponseNeedle);
      for (const needle of requiredNeedles) {
        assertContains(path, needle);
      }
      for (const needle of forbiddenNeedles) {
        assertNotContains(path, needle);
      }
    }
  };

  const assertGranularProviderBoundary = (policy) => {
    const {
      protoPath,
      descriptorMessage = "IdentityProviderDescriptor",
      protocolVersionType = "IdentityProtocolVersion",
      capabilityType = "IdentityProviderCapability",
      requestMessage = "IdentityProviderRequest",
      resultMessage = "IdentityProviderResult",
      errorMessage = "IdentityProviderError",
      errorReasonType = "IdentityProviderErrorReason",
      responseMessage = "IdentityProviderResponse",
      allowServices = false,
      codecPath,
      runtimePath,
      requiredCodecNeedles = [],
      requiredRuntimeNeedles = [],
      operations,
      adapters = [],
      retiredPaths = [],
    } = policy ?? {};
    for (const [name, value] of Object.entries({ protoPath, codecPath, runtimePath })) {
      if (typeof value !== "string" || value.length === 0) {
        fail(`granular provider boundary policy ${name} must be a non-empty string`);
      }
    }
    for (const [name, value] of Object.entries({
      descriptorMessage,
      requestMessage,
      resultMessage,
      errorMessage,
      responseMessage,
    })) {
      if (typeof value !== "string" || !/^[A-Za-z_][A-Za-z0-9_]*$/u.test(value)) {
        fail(`granular provider boundary policy ${name} must be a protobuf identifier`);
      }
    }
    for (const [name, value] of Object.entries({
      protocolVersionType,
      capabilityType,
      errorReasonType,
    })) {
      if (typeof value !== "string" || !/^\.?[A-Za-z_][A-Za-z0-9_.]*$/u.test(value)) {
        fail(`granular provider boundary policy ${name} must be a protobuf type name`);
      }
    }
    if (typeof allowServices !== "boolean") {
      fail("granular provider boundary allowServices must be a boolean");
    }
    for (const [name, needles] of Object.entries({
      requiredCodecNeedles,
      requiredRuntimeNeedles,
    })) {
      if (
        !Array.isArray(needles) ||
        needles.some((needle) => typeof needle !== "string" || needle.length === 0)
      ) {
        fail(`granular provider boundary ${name} must be an array of non-empty strings`);
      }
    }
    if (!Array.isArray(operations) || operations.length === 0) {
      fail("granular provider boundary operations must be a non-empty array");
    }
    const operationFieldNames = new Set();
    const operationNumbers = new Set();
    for (const [index, operation] of operations.entries()) {
      if (operation === null || typeof operation !== "object" || Array.isArray(operation)) {
        fail(`granular provider boundary operations[${index}] must be an object`);
      }
      const { fieldName, requestType, resultType, number } = operation;
      for (const [name, value] of Object.entries({ fieldName, requestType, resultType })) {
        if (typeof value !== "string" || !/^[A-Za-z_][A-Za-z0-9_]*$/u.test(value)) {
          fail(`granular provider boundary operations[${index}].${name} must be a protobuf identifier`);
        }
      }
      if (!Number.isSafeInteger(number) || number < 1 || number > 536_870_911) {
        fail(`granular provider boundary operations[${index}].number must be a protobuf field number`);
      }
      if (operationFieldNames.has(fieldName)) {
        fail(`granular provider boundary operation field ${fieldName} is duplicated`);
      }
      if (operationNumbers.has(number)) {
        fail(`granular provider boundary operation field number ${number} is duplicated`);
      }
      operationFieldNames.add(fieldName);
      operationNumbers.add(number);
    }
    if (
      !Array.isArray(adapters) ||
      adapters.some(
        (adapter) =>
          adapter === null || typeof adapter !== "object" || Array.isArray(adapter),
      )
    ) {
      fail("granular provider boundary adapters must be an array of objects");
    }

    assertProtoContract(protoPath);
    const proto = stripProtoLineComments(readText(protoPath));
    if (!allowServices && extractProtoBlocks(proto, "service").length !== 0) {
      fail(`${protoPath} must define provider messages without protobuf services`);
    }
    const messages = new Map(
      extractProtoBlocks(proto, "message").map((block) => [block.name, block.body]),
    );
    const parseProtoFields = (body) =>
      [...body.matchAll(
        /^\s*(?:(optional|required|repeated)\s+)?((?:\.?[A-Za-z_][A-Za-z0-9_.]*)|(?:map\s*<\s*[A-Za-z_][A-Za-z0-9_.]*\s*,\s*\.?[A-Za-z_][A-Za-z0-9_.]*\s*>))\s+([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(\d+)\s*(?:\[[^\]]*\])?\s*;/gmu,
      )].map((match) => ({
        label: match[1] ?? null,
        type: match[2],
        name: match[3],
        number: Number.parseInt(match[4], 10),
      }));
    const hasExactField = (fields, expected) =>
      fields.some(
        (field) =>
          field.label === expected.label &&
          field.type === expected.type &&
          field.name === expected.name &&
          field.number === expected.number,
      );
    const descriptor = messages.get(descriptorMessage);
    const descriptorFields = descriptor === undefined ? [] : parseProtoFields(descriptor);
    if (
      descriptor === undefined ||
      descriptorFields.length !== 3 ||
      !hasExactField(descriptorFields, {
        label: null,
        type: protocolVersionType,
        name: "protocol_version",
        number: 1,
      }) ||
      !hasExactField(descriptorFields, {
        label: "repeated",
        type: capabilityType,
        name: "capabilities",
        number: 2,
      }) ||
      !hasExactField(descriptorFields, {
        label: null,
        type: "uint32",
        name: "max_response_bytes",
        number: 3,
      })
    ) {
      fail(`${protoPath} ${descriptorMessage} must define version, capabilities, and response bound`);
    }
    const request = messages.get(requestMessage);
    const operationOneof =
      request === undefined
        ? undefined
        : extractProtoBlocks(request, "oneof").find((block) => block.name === "operation");
    if (
      request === undefined ||
      parseProtoFields(request).length !== operations.length + 2 ||
      !/^\s*uint64\s+executor_id\s*=\s*1\s*;/mu.test(request) ||
      !/^\s*uint64\s+sequence\s*=\s*2\s*;/mu.test(request) ||
      operationOneof === undefined
    ) {
      fail(`${protoPath} ${requestMessage} must define correlated granular operations`);
    }
    const result = messages.get(resultMessage);
    const resultOneof =
      result === undefined
        ? undefined
        : extractProtoBlocks(result, "oneof").find((block) => block.name === "result");
    if (result === undefined || resultOneof === undefined) {
      fail(`${protoPath} ${resultMessage} must define oneof result`);
    }
    const requestFields = parseProtoFields(operationOneof.body);
    const resultFields = parseProtoFields(resultOneof.body);
    if (requestFields.length !== operations.length || resultFields.length !== operations.length) {
      fail(`${protoPath} provider request and result oneofs must contain only declared granular operations`);
    }
    for (const operation of operations) {
      const requestField = requestFields.find((field) => field.name === operation.fieldName);
      const resultField = resultFields.find((field) => field.name === operation.fieldName);
      if (
        requestField === undefined ||
        requestField.type !== operation.requestType ||
        requestField.number !== operation.number ||
        resultField === undefined ||
        resultField.type !== operation.resultType ||
        resultField.number !== operation.number
      ) {
        fail(`${protoPath} provider operation ${operation.fieldName} must match its declared request, result, and field number`);
      }
    }
    const providerError = messages.get(errorMessage);
    const providerErrorFields =
      providerError === undefined ? [] : parseProtoFields(providerError);
    if (
      providerError === undefined ||
      providerErrorFields.length !== 1 ||
      !hasExactField(providerErrorFields, {
        label: null,
        type: errorReasonType,
        name: "reason",
        number: 1,
      })
    ) {
      fail(`${protoPath} ${errorMessage} must define one typed reason field`);
    }
    const response = messages.get(responseMessage);
    const outcomeOneof =
      response === undefined
        ? undefined
        : extractProtoBlocks(response, "oneof").find((block) => block.name === "outcome");
    const outcomeFields = outcomeOneof === undefined ? [] : parseProtoFields(outcomeOneof.body);
    if (
      response === undefined ||
      parseProtoFields(response).length !== 4 ||
      !/^\s*uint64\s+executor_id\s*=\s*1\s*;/mu.test(response) ||
      !/^\s*uint64\s+sequence\s*=\s*2\s*;/mu.test(response) ||
      outcomeOneof === undefined ||
      outcomeFields.length !== 2 ||
      !new RegExp(`^\\s*${resultMessage}\\s+result\\s*=\\s*3\\s*;`, "mu").test(
        response,
      ) ||
      !new RegExp(`^\\s*${errorMessage}\\s+error\\s*=\\s*4\\s*;`, "mu").test(
        response,
      )
    ) {
      fail(`${protoPath} ${responseMessage} must echo correlation and define typed outcomes`);
    }

    for (const needle of requiredCodecNeedles) {
      assertContains(codecPath, needle);
    }
    for (const needle of requiredRuntimeNeedles) {
      assertContains(runtimePath, needle);
    }
    for (const [index, adapter] of adapters.entries()) {
      const { path, requiredNeedles = [], forbiddenNeedles = [] } = adapter;
      if (typeof path !== "string" || path.length === 0) {
        fail(`granular provider boundary adapters[${index}].path must be a non-empty string`);
      }
      for (const [name, needles] of Object.entries({ requiredNeedles, forbiddenNeedles })) {
        if (
          !Array.isArray(needles) ||
          needles.some((needle) => typeof needle !== "string" || needle.length === 0)
        ) {
          fail(
            `granular provider boundary adapters[${index}].${name} must be an array of non-empty strings`,
          );
        }
      }
      for (const needle of requiredNeedles) {
        assertContains(path, needle);
      }
      for (const needle of forbiddenNeedles) {
        assertNotContains(path, needle);
      }
    }
    assertPathsAbsent(retiredPaths);
  };

  return {
    root,
    fail,
    readText,
    readJson,
    listFiles,
    requireTracked,
    assertPathsAbsent,
    loadTrackedFiles,
    assertContains,
    assertNotContains,
    assertMinOccurrences,
    assertTextPolicy,
    requireMatch,
    assertNotMatches,
    assertLockPackageVersion,
    run,
    runCommands,
    runNodeCheck,
    packageList,
    assertPackageFiles,
    assertCargoMetadataDocument,
    assertCargoMetadataPolicy,
    assertCargoWorkspacePolicy,
    assertRepositoryShapePolicy,
    assertRustSourcePolicy,
    assertTypeScriptSourcePolicy,
    assertSwiftSourcePolicy,
    assertKotlinSourcePolicy,
    snapshotDirectory,
    assertSnapshotsEqual,
    validateGeneratedArtifactsPolicy,
    assertGeneratedArtifactsFresh,
    assertGeneratedProtoHardeningPolicy,
    assertReallyMeProtobufReleasePolicy,
    assertReallyMeVendoredCorePolicy,
    assertNodeWorkflowJobsPinNode,
    assertWorkflowActionsPinned,
    assertWorkflowPermissionsPolicy,
    assertCargoFuzzWorkflowPolicy,
    assertReallyMeRustProtoRepositoryPolicy,
    normalizeWorkflowRunCommand,
    extractWorkflowSteps,
    assertWorkflowRunStep,
    assertWorkflowUsesStep,
    assertWorkflowPolicy,
    stripProtoLineComments,
    extractProtoBlocks,
    assertProtoContract,
    assertReallyMeOperationBoundaryContract,
    assertGranularProviderBoundary,
    assertSpdxHeaders,
  };
}
