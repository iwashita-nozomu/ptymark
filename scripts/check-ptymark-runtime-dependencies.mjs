#!/usr/bin/env node

// @dependency-start
// contract tool
// responsibility Verifies that ptymark runtime and renderer dependency pins remain aligned.
// upstream configuration ../Cargo.toml Rust compatibility floor
// upstream environment ../rust-toolchain.toml canonical Rust toolchain
// upstream environment ../renderers/managed-bundle.env managed runtime and renderer versions
// upstream configuration ../renderers/package.json direct renderer dependency declarations
// upstream configuration ../renderers/package-lock.json exact renderer dependency graph
// upstream environment ../docker/ptymark-versions.env canonical product image inputs
// upstream environment ../docker/ptymark.Dockerfile canonical product validation image
// upstream environment ../docker/ptymark-compose.yaml product container entrypoint
// upstream workflow ../.github/workflows/ptymark-ci.yml product acceptance matrix
// upstream workflow ../.github/workflows/ptymark-release.yml release build matrix
// upstream workflow ../.github/workflows/ptymark-dependency-alignment.yml focused dependency gate
// upstream configuration ../ptymark.mk canonical product validation commands
// downstream workflow ../.github/workflows/ptymark-dependency-alignment.yml runs this contract on dependency changes
// @dependency-end

import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(
  process.env.PTYMARK_DEPENDENCY_ROOT ?? path.join(scriptDirectory, ".."),
);
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repositoryRoot, relativePath);
  try {
    return fs.readFileSync(absolutePath, "utf8");
  } catch (error) {
    failures.push(`${relativePath}: unable to read (${error.message})`);
    return "";
  }
}

function capture(text, pattern, description) {
  const match = text.match(pattern);
  if (!match) {
    failures.push(`missing ${description}`);
    return null;
  }
  return match[1];
}

function parseEnvironment(relativePath, text) {
  const values = new Map();
  for (const [index, rawLine] of text.split(/\r?\n/u).entries()) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) {
      continue;
    }
    const separator = line.indexOf("=");
    if (separator <= 0) {
      failures.push(`${relativePath}:${index + 1}: expected KEY=VALUE`);
      continue;
    }
    const key = line.slice(0, separator).trim();
    const value = line.slice(separator + 1).trim();
    if (!/^[A-Z][A-Z0-9_]*$/u.test(key)) {
      failures.push(`${relativePath}:${index + 1}: invalid key ${JSON.stringify(key)}`);
      continue;
    }
    if (values.has(key)) {
      failures.push(`${relativePath}:${index + 1}: duplicate key ${key}`);
      continue;
    }
    if (value === "") {
      failures.push(`${relativePath}:${index + 1}: empty value for ${key}`);
      continue;
    }
    values.set(key, value);
  }
  return values;
}

function requiredEnvironmentValue(values, relativePath, key) {
  const value = values.get(key);
  if (value === undefined) {
    failures.push(`${relativePath}: missing ${key}`);
    return null;
  }
  return value;
}

function expectEqual(description, expected, actual) {
  if (expected === null || expected === undefined) {
    return;
  }
  if (actual === null || actual === undefined) {
    failures.push(`${description}: expected ${JSON.stringify(expected)}, found no value`);
    return;
  }
  if (actual !== expected) {
    failures.push(`${description}: expected ${JSON.stringify(expected)}, found ${JSON.stringify(actual)}`);
  }
}

function expectEvery(description, expected, values) {
  if (expected === null) {
    return;
  }
  if (values.length === 0) {
    failures.push(`${description}: no version pins found`);
    return;
  }
  values.forEach((value, index) => {
    expectEqual(`${description} #${index + 1}`, expected, value);
  });
}

function parseJson(relativePath, text) {
  try {
    return JSON.parse(text);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON (${error.message})`);
    return {};
  }
}

function imageNodeVersion(image) {
  if (image === null) {
    return null;
  }
  const match = image.match(/^node:(\d+\.\d+\.\d+)(?:[-@].*)?$/u);
  if (!match) {
    failures.push(
      `docker/ptymark-versions.env: NODE_IMAGE must use node:<semver>-<variant>, found ${JSON.stringify(image)}`,
    );
    return null;
  }
  return match[1];
}

function collectMatches(text, pattern) {
  return [...text.matchAll(pattern)].map((match) => match[1]);
}

const cargoToml = read("Cargo.toml");
const rustToolchainToml = read("rust-toolchain.toml");
const managedBundlePath = "renderers/managed-bundle.env";
const managedBundle = parseEnvironment(managedBundlePath, read(managedBundlePath));
const packageJsonPath = "renderers/package.json";
const packageLockPath = "renderers/package-lock.json";
const packageJson = parseJson(packageJsonPath, read(packageJsonPath));
const packageLock = parseJson(packageLockPath, read(packageLockPath));
const dockerVersionsPath = "docker/ptymark-versions.env";
const dockerVersions = parseEnvironment(dockerVersionsPath, read(dockerVersionsPath));
const dockerfile = read("docker/ptymark.Dockerfile");
const compose = read("docker/ptymark-compose.yaml");
const ciWorkflow = read(".github/workflows/ptymark-ci.yml");
const releaseWorkflow = read(".github/workflows/ptymark-release.yml");
const dependencyWorkflow = read(".github/workflows/ptymark-dependency-alignment.yml");
const productMakefile = read("ptymark.mk");

const rustVersion = capture(
  rustToolchainToml,
  /^channel\s*=\s*"([^"]+)"\s*$/mu,
  "rust-toolchain.toml toolchain channel",
);
expectEqual(
  "Cargo.toml package.rust-version",
  rustVersion,
  capture(cargoToml, /^rust-version\s*=\s*"([^"]+)"\s*$/mu, "Cargo.toml package.rust-version"),
);
expectEqual(
  `${dockerVersionsPath} RUST_VERSION`,
  rustVersion,
  requiredEnvironmentValue(dockerVersions, dockerVersionsPath, "RUST_VERSION"),
);
expectEqual(
  "docker/ptymark.Dockerfile ARG RUST_VERSION",
  rustVersion,
  capture(
    dockerfile,
    /^ARG RUST_VERSION=([^\s]+)\s*$/mu,
    "docker/ptymark.Dockerfile ARG RUST_VERSION",
  ),
);
expectEqual(
  "docker/ptymark-compose.yaml RUST_VERSION default",
  rustVersion,
  capture(
    compose,
    /^\s*RUST_VERSION:\s*\$\{RUST_VERSION:-([^}]+)\}\s*$/mu,
    "docker/ptymark-compose.yaml RUST_VERSION default",
  ),
);
for (const [workflowName, workflow] of [
  [".github/workflows/ptymark-ci.yml", ciWorkflow],
  [".github/workflows/ptymark-release.yml", releaseWorkflow],
]) {
  expectEvery(
    `${workflowName} Rust toolchain install`,
    rustVersion,
    collectMatches(workflow, /rustup toolchain install\s+([^\s]+)\s+/gu),
  );
  expectEvery(
    `${workflowName} Rust override`,
    rustVersion,
    collectMatches(workflow, /rustup override set\s+([^\s]+)\s*$/gmu),
  );
}

const managedBundleVersion = requiredEnvironmentValue(
  managedBundle,
  managedBundlePath,
  "PTYMARK_MANAGED_BUNDLE_VERSION",
);
if (managedBundleVersion !== null && !/^\d+$/u.test(managedBundleVersion)) {
  failures.push(`${managedBundlePath}: PTYMARK_MANAGED_BUNDLE_VERSION must be an integer`);
}
const managedNodeVersion = requiredEnvironmentValue(
  managedBundle,
  managedBundlePath,
  "PTYMARK_MANAGED_NODE_VERSION",
);
const nodeImage = requiredEnvironmentValue(dockerVersions, dockerVersionsPath, "NODE_IMAGE");
expectEqual(`${dockerVersionsPath} NODE_IMAGE version`, managedNodeVersion, imageNodeVersion(nodeImage));
expectEqual(
  "docker/ptymark.Dockerfile ARG NODE_IMAGE",
  nodeImage,
  capture(dockerfile, /^ARG NODE_IMAGE=([^\s]+)\s*$/mu, "docker/ptymark.Dockerfile ARG NODE_IMAGE"),
);
expectEqual(
  "docker/ptymark-compose.yaml NODE_IMAGE default",
  nodeImage,
  capture(
    compose,
    /^\s*NODE_IMAGE:\s*\$\{NODE_IMAGE:-([^}]+)\}\s*$/mu,
    "docker/ptymark-compose.yaml NODE_IMAGE default",
  ),
);

const managedNodeMajor =
  managedNodeVersion === null ? null : Number.parseInt(managedNodeVersion.split(".")[0], 10);
const expectedNodeRange =
  managedNodeMajor === null ? null : `>=${managedNodeVersion} <${managedNodeMajor + 1}`;
expectEqual(`${packageJsonPath} engines.node`, expectedNodeRange, packageJson.engines?.node ?? null);
const lockRoot = packageLock.packages?.[""];
if (lockRoot === undefined) {
  failures.push(`${packageLockPath}: missing packages[\"\"] root metadata`);
}
expectEqual(`${packageLockPath} root name`, packageJson.name ?? null, lockRoot?.name ?? null);
expectEqual(`${packageLockPath} root version`, packageJson.version ?? null, lockRoot?.version ?? null);
expectEqual(
  `${packageLockPath} root engines.node`,
  packageJson.engines?.node ?? null,
  lockRoot?.engines?.node ?? null,
);
for (const [workflowName, workflow] of [
  [".github/workflows/ptymark-ci.yml", ciWorkflow],
  [".github/workflows/ptymark-dependency-alignment.yml", dependencyWorkflow],
]) {
  expectEvery(
    `${workflowName} Node setup`,
    managedNodeVersion,
    collectMatches(workflow, /^\s*node-version:\s*['"]?([^'"\s]+)['"]?\s*$/gmu),
  );
}

const managedMathJaxVersion = requiredEnvironmentValue(
  managedBundle,
  managedBundlePath,
  "PTYMARK_MANAGED_MATHJAX_VERSION",
);
const managedRendererVersions = new Map([
  [
    "@mermaid-js/mermaid-cli",
    requiredEnvironmentValue(managedBundle, managedBundlePath, "PTYMARK_MANAGED_MERMAID_VERSION"),
  ],
  ["@mathjax/src", managedMathJaxVersion],
  ["@mathjax/mathjax-newcm-font", managedMathJaxVersion],
  [
    "puppeteer",
    requiredEnvironmentValue(managedBundle, managedBundlePath, "PTYMARK_MANAGED_PUPPETEER_VERSION"),
  ],
]);
const directDependencies = packageJson.dependencies ?? {};
const lockedDirectDependencies = lockRoot?.dependencies ?? {};
for (const [name, version] of managedRendererVersions) {
  expectEqual(`${packageJsonPath} dependencies.${name}`, version, directDependencies[name] ?? null);
  expectEqual(
    `${packageLockPath} root dependencies.${name}`,
    version,
    lockedDirectDependencies[name] ?? null,
  );
}
for (const name of Object.keys(directDependencies)) {
  if (!managedRendererVersions.has(name)) {
    failures.push(`${managedBundlePath}: no managed version key covers direct dependency ${name}`);
  }
}
for (const name of managedRendererVersions.keys()) {
  if (!(name in directDependencies)) {
    failures.push(`${packageJsonPath}: managed dependency ${name} is not declared`);
  }
}

for (const redundantKey of ["MERMAID_CLI_VERSION", "MATHJAX_VERSION", "PUPPETEER_VERSION"]) {
  if (dockerVersions.has(redundantKey)) {
    failures.push(`${dockerVersionsPath}: ${redundantKey} duplicates ${managedBundlePath}; remove it`);
  }
}

const installBlock = capture(
  dockerfile,
  /apt-get install --yes --no-install-recommends\s+([\s\S]*?)\s+&& rm -rf \/var\/lib\/apt\/lists\/\*/u,
  "docker/ptymark.Dockerfile apt dependency block",
);
if (installBlock !== null) {
  const installedPackages = new Set(
    installBlock
      .replaceAll("\\", " ")
      .split(/\s+/u)
      .filter(Boolean),
  );
  for (const name of ["chafa", "chromium", "lua5.4", "shellcheck"]) {
    if (!installedPackages.has(name)) {
      failures.push(`docker/ptymark.Dockerfile: missing system dependency ${name}`);
    }
  }
}
expectEqual(
  "docker/ptymark.Dockerfile PUPPETEER_SKIP_DOWNLOAD",
  "true",
  capture(dockerfile, /\bPUPPETEER_SKIP_DOWNLOAD=([^\s\\]+)/u, "PUPPETEER_SKIP_DOWNLOAD"),
);
expectEqual(
  "docker/ptymark.Dockerfile PUPPETEER_EXECUTABLE_PATH",
  "/usr/bin/chromium",
  capture(dockerfile, /\bPUPPETEER_EXECUTABLE_PATH=([^\s\\]+)/u, "PUPPETEER_EXECUTABLE_PATH"),
);

if (!/^ptymark-runtime-dependencies:\s*$/mu.test(productMakefile)) {
  failures.push("ptymark.mk: missing ptymark-runtime-dependencies target");
}
if (!/\$\(MAKE\) ptymark-runtime-dependencies/u.test(productMakefile)) {
  failures.push("ptymark.mk: ptymark-check-local does not run ptymark-runtime-dependencies");
}
if (!/run:\s*make ptymark-runtime-dependencies/u.test(dependencyWorkflow)) {
  failures.push(
    ".github/workflows/ptymark-dependency-alignment.yml: dependency check command is not wired",
  );
}

if (failures.length > 0) {
  console.error("ptymark runtime dependency alignment: failed");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exitCode = 1;
} else {
  console.log(
    `ptymark runtime dependency alignment: ok (Rust ${rustVersion}, Node ${managedNodeVersion}, ${managedRendererVersions.size} renderer packages)`,
  );
}
