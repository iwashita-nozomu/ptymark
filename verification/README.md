<!--
@dependency-start
contract reference
responsibility Explains verification evidence, reproduction, and merge policy.
upstream design ./manifest.toml required checks
upstream implementation ../.github/workflows/ptymark-ci.yml product evidence
downstream implementation ../tests/verification_manifest_contract.rs drift prevention
@dependency-end
-->

# ptymark verification catalog

This directory is the Git-owned source of truth for **what must be verified** before a ptymark change is merged. Run-specific pass/fail data belongs to GitHub Actions for the tested commit; definitions, commands, fixtures, compatibility inventories, and evidence names belong in the repository.

## Files

- `manifest.toml` lists every required check, owner, level, platform, command, source files, and expected evidence.
- `tests/verification_manifest_contract.rs` validates the catalog on every Rust test run.
- `compat/shell-integrations/*.tsv` contains the reviewed shell-integration inventory.
- `documents/shell-plugin-compatibility.md` explains what `contract-verified` means and what it does not mean.
- `.github/workflows/ptymark-ci.yml` is the canonical product workflow.
- `.github/workflows/ci.yml` performs a lightweight repository-wiring check; all feature evidence is owned by the product workflow.

## Evidence levels

| Level | Meaning |
| --- | --- |
| `static` | Formatting, linting, parser, syntax, or manifest validation without executing the feature path. |
| `contract` | Deterministic unit/integration contract with controlled fixtures. |
| `integration` | Real installer, renderer, browser, process, or adapter execution. |
| `acceptance` | One complete isolated user or release journey through the installed product and its real dependencies. |
| `package` | A native executable is built and packaged in an ephemeral CI workspace, locally installed and smoke-tested, then discarded without upload or distribution. |
| `compatibility` | Terminal behavior and shell ownership are validated without claiming every upstream release is vendored. |
| `repository` | Lightweight repository wiring that is independent of feature behavior. |

## Product checks

The following IDs are stable review references. Renaming or removing one requires an explicit manifest and test review.

### Catalog and Rust quality

- `catalog.schema`
- `rust.format`
- `rust.clippy`
- `rust.tests`

### Terminal, detection, pipeline, routing, and cache

- `terminal.byte-exact`
- `terminal.alternate-screen`
- `detector.explicit-fences`
- `pipeline.single-commit`
- `runtime.pipeline-factory`
- `runtime.stream-pump`
- `command.filtered-run`
- `pty.native-session`
- `routing.typed-handoff`
- `cache.contract`
- `config.contract`

### Rendering engines

- `engine.executable-resolution`
- `engine.user-installed`
- `engine.managed-docker`
- `engine.managed-windows`

### Installer entrypoints

- `installer.unix`
- `installer.powershell`
- `installer.cmd`
- `installer.winbash`

### Fresh-source end-to-end acceptance

- `acceptance.fresh-source-linux`

### Shell and WezTerm coexistence

- `shell.inventory`
- `shell.behavior-profiles`
- `shell.profile-coexistence-unix`
- `shell.profile-coexistence-windows`
- `wezterm.plugin`

### Scripts and ephemeral local package smoke

- `scripts.syntax`
- `package-smoke.linux`
- `package-smoke.macos`
- `package-smoke.windows`

## Canonical commands

Fast Rust contract gate:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Canonical Docker product gate:

```bash
make ptymark-check
```

Installer and shell coexistence on Unix:

```bash
cargo build --locked
bash tests/install_smoke.sh
bash tests/shell_profile_coexistence.sh target/debug/ptymark
```

Managed renderer path in the canonical image:

```bash
PTYMARK_TEST_BROWSER=/usr/bin/chromium \
PTYMARK_BROWSER_NO_SANDBOX=1 \
bash tests/managed_renderer_smoke.sh
```

Fresh-source installation through the normal source installer, private Node runtime, real managed engines, strict preview, and native PTY:

```bash
PTYMARK_TEST_BROWSER=/usr/bin/chromium \
PTYMARK_BROWSER_NO_SANDBOX=1 \
bash tests/fresh_source_acceptance.sh
```

This CI/release acceptance proves the documented source wiring. It does not define or replace a future user-facing guided setup or self-test.

Windows-native checks are run by GitHub Actions using `installer.ps1`, `installer.cmd`, Git Bash path conversion, the managed renderer smoke, shell-profile coexistence, and `package-release.ps1`.

## Merge policy

A PR is ready only when all entries with `required = true` are represented by a successful check for the current head commit. A successful older commit is not evidence for a moved head. Diagnostic `*-once.yml` workflows are temporary tools and must be removed before the PR is marked ready.

The product workflow uploads named evidence for failure-prone integration paths:

- `ptymark-shellcheck-evidence`
- `ptymark-selected-renderer-evidence`
- `ptymark-windows-managed-renderer-evidence`
Executable package outputs are intentionally not uploaded. Only bounded diagnostic logs or non-executable evidence may be retained as workflow artifacts.

Non-executable artifacts may support diagnosis, but the committed tests and manifest define the contract. Project-built executable artifacts are not a distribution channel.
