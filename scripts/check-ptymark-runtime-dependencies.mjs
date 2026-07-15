#!/usr/bin/env node

// @dependency-start
// contract tool
// responsibility Verifies that shipped ptymark and managed-renderer dependency pins remain aligned.
// upstream configuration ../Cargo.toml Rust compatibility floor
// upstream environment ../rust-toolchain.toml canonical Rust toolchain
// upstream environment ../renderers/managed-bundle.env managed runtime and renderer versions
// upstream configuration ../renderers/package.json direct renderer dependency declarations
// upstream configuration ../renderers/package-lock.json exact renderer dependency graph
// upstream workflow ../.github/workflows/ptymark-ci.yml product acceptance matrix
// upstream workflow ../.github/workflows/ptymark-dependency-alignment.yml focused dependency gate
// upstream configuration ../ptymark.mk canonical product validation commands
// downstream workflow ../.github/workflows/ptymark-dependency-alignment.yml runs this contract on dependency changes
// @dependency-end

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(
  process.env.PTYMARK_DEPENDENCY_ROOT
    ?? path.join(path.dirname(fileURLToPath(import.meta.url)), ".."),
);
const failures = [];

function read(relativePath) {
  try {
    return fs.readFileSync(path.join(root, relativePath), "utf8");
  } catch (error) {
    failures.push(`${relativePath}: unable to read (${error.message})`);
    return "";
  }
}

function capture(text, pattern, description) {
  const match = text.match(pattern);
  if (match === null) {
    failures.push(`missing ${description}`);
    return null;
  }
  return match[1];
}

function matches(text, pattern) {
  return [...text.matchAll(pattern)].map((match) => match[1]);
}

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON (${error.message})`);
    return {};
  }
}

function parseEnvironment(relativePath) {
  const values = new Map();
  for (const [index, rawLine] of read(relativePath).split(/\r?\n/u).entries()) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) {
      continue;
    }
    const separator = line.indexOf("=");
    const key = separator < 0 ? "" : line.slice(0, separator).trim();
    const value = separator < 0 ? "" : line.slice(separator + 1).trim();
    if (!/^[A-Z][A-Z0-9_]*$/u.test(key) || value === "") {
      failures.push(`${relativePath}:${index + 1}: expected non-empty KEY=VALUE`);
    } else if (values.has(key)) {
      failures.push(`${relativePath}:${index + 1}: duplicate key ${key}`);
    } else {
      values.set(key, value);
    }
  }
  return values;
}

function required(values, relativePath, key) {
  const value = values.get(key);
  if (value === undefined) {
    failures.push(`${relativePath}: missing ${key}`);
    return null;
  }
  return value;
}

function equal(description, expected, actual) {
  if (expected === null || expected === undefined) {
    return;
  }
  if (actual !== expected) {
    failures.push(
      `${description}: expected ${JSON.stringify(expected)}, found ${JSON.stringify(actual)}`,
    );
  }
}

function every(description, expected, actualValues) {
  if (expected === null) {
    return;
  }
  if (actualValues.length === 0) {
    failures.push(`${description}: no version pins found`);
  }
  actualValues.forEach((actual, index) => equal(`${description} #${index + 1}`, expected, actual));
}

const cargo = read("Cargo.toml");
const toolchain = read("rust-toolchain.toml");
const bundlePath = "renderers/managed-bundle.env";
const bundle = parseEnvironment(bundlePath);
const packagePath = "renderers/package.json";
const lockPath = "renderers/package-lock.json";
const packageJson = parseJson(packagePath);
const packageLock = parseJson(lockPath);
const ci = read(".github/workflows/ptymark-ci.yml");
const focusedWorkflow = read(".github/workflows/ptymark-dependency-alignment.yml");
const makefile = read("ptymark.mk");

const rustVersion = capture(
  toolchain,
  /^channel\s*=\s*"([^"]+)"\s*$/mu,
  "rust-toolchain.toml toolchain channel",
);
equal(
  "Cargo.toml package.rust-version",
  rustVersion,
  capture(cargo, /^rust-version\s*=\s*"([^"]+)"\s*$/mu, "Cargo.toml package.rust-version"),
);
for (const [name, workflow] of [
  [".github/workflows/ptymark-ci.yml", ci],
]) {
  every(
    `${name} Rust toolchain install`,
    rustVersion,
    matches(workflow, /rustup toolchain install\s+([^\s]+)\s+/gu),
  );
  every(
    `${name} Rust override`,
    rustVersion,
    matches(workflow, /rustup override set\s+([^\s]+)\s*$/gmu),
  );
}

const bundleVersion = required(bundle, bundlePath, "PTYMARK_MANAGED_BUNDLE_VERSION");
if (bundleVersion !== null && !/^\d+$/u.test(bundleVersion)) {
  failures.push(`${bundlePath}: PTYMARK_MANAGED_BUNDLE_VERSION must be an integer`);
}

const nodeVersion = required(bundle, bundlePath, "PTYMARK_MANAGED_NODE_VERSION");
const nodeMajor = nodeVersion === null ? null : Number.parseInt(nodeVersion.split(".")[0], 10);
const expectedNodeRange = nodeMajor === null ? null : `>=${nodeVersion} <${nodeMajor + 1}`;
equal(`${packagePath} engines.node`, expectedNodeRange, packageJson.engines?.node);

const lockRoot = packageLock.packages?.[""];
if (lockRoot === undefined) {
  failures.push(`${lockPath}: missing packages[""] root metadata`);
}
equal(`${lockPath} root name`, packageJson.name, lockRoot?.name);
equal(`${lockPath} root version`, packageJson.version, lockRoot?.version);
equal(`${lockPath} root engines.node`, packageJson.engines?.node, lockRoot?.engines?.node);

for (const [name, workflow] of [
  [".github/workflows/ptymark-ci.yml", ci],
  [".github/workflows/ptymark-dependency-alignment.yml", focusedWorkflow],
]) {
  every(
    `${name} Node setup`,
    nodeVersion,
    matches(workflow, /^\s*node-version:\s*['"]?([^'"\s]+)['"]?\s*$/gmu),
  );
}

const mathjaxVersion = required(bundle, bundlePath, "PTYMARK_MANAGED_MATHJAX_VERSION");
const managedPackages = new Map([
  [
    "@mermaid-js/mermaid-cli",
    required(bundle, bundlePath, "PTYMARK_MANAGED_MERMAID_VERSION"),
  ],
  ["@mathjax/src", mathjaxVersion],
  ["@mathjax/mathjax-newcm-font", mathjaxVersion],
  ["puppeteer", required(bundle, bundlePath, "PTYMARK_MANAGED_PUPPETEER_VERSION")],
]);
const directDependencies = packageJson.dependencies ?? {};
const lockedDependencies = lockRoot?.dependencies ?? {};

for (const [name, version] of managedPackages) {
  equal(`${packagePath} dependencies.${name}`, version, directDependencies[name]);
  equal(`${lockPath} root dependencies.${name}`, version, lockedDependencies[name]);
}
for (const name of Object.keys(directDependencies)) {
  if (!managedPackages.has(name)) {
    failures.push(`${bundlePath}: no managed version key covers direct dependency ${name}`);
  }
}
for (const name of managedPackages.keys()) {
  if (!(name in directDependencies)) {
    failures.push(`${packagePath}: managed dependency ${name} is not declared`);
  }
}

if (!/^ptymark-runtime-dependencies:\s*$/mu.test(makefile)) {
  failures.push("ptymark.mk: missing ptymark-runtime-dependencies target");
}
if (!/\$\(MAKE\) ptymark-runtime-dependencies/u.test(makefile)) {
  failures.push("ptymark.mk: ptymark-check-local does not run ptymark-runtime-dependencies");
}
if (!/run:\s*make ptymark-runtime-dependencies/u.test(focusedWorkflow)) {
  failures.push(
    ".github/workflows/ptymark-dependency-alignment.yml: dependency check command is not wired",
  );
}

if (failures.length > 0) {
  console.error("ptymark product dependency alignment: failed");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exitCode = 1;
} else {
  console.log(
    `ptymark product dependency alignment: ok (Rust ${rustVersion}, Node ${nodeVersion}, ${managedPackages.size} renderer packages)`,
  );
}
