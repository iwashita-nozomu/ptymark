"""Remove inherited AgentCanon/template surfaces and leave a product-only Ptymark tree."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=ROOT,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def tracked_paths() -> set[str]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
    )
    return {item.decode("utf-8") for item in result.stdout.split(b"\0") if item}


def remove(path: str) -> None:
    if path in tracked_paths() or any(
        tracked == path or tracked.startswith(f"{path}/") for tracked in tracked_paths()
    ):
        subprocess.run(
            ["git", "rm", "-r", "-f", "--ignore-unmatch", "--", path],
            cwd=ROOT,
            check=True,
        )


def write(path: str, content: str) -> None:
    target = ROOT / path
    if target.is_symlink():
        target.unlink()
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content.rstrip() + "\n", encoding="utf-8")


def remove_agent_links() -> None:
    for path in sorted(tracked_paths()):
        target = ROOT / path
        if target.is_symlink():
            link = os.readlink(target)
            if "agent-canon" in link.lower() or "agent_canon" in link.lower():
                remove(path)

    for path in sorted(tracked_paths()):
        lowered = path.lower()
        if "agent-canon" in lowered or "agent_canon" in lowered:
            remove(path)


def remove_template_surfaces() -> None:
    for path in [
        ".gitmodules",
        "vendor/agent-canon",
        ".agent-canon",
        ".agents",
        "agents",
        ".codex",
        "memory",
        "notes",
        "evidence",
        "experiments",
        "python",
        "tests/agent_tools",
        "pyproject.toml",
        "pyrightconfig.json",
        "responsibility-scope.toml",
        ".devcontainer",
        "QUICK_START.md",
        ".github/workflows/agent-improvement-guide.yml",
        ".github/workflows/agent-coordination.yml",
        ".github/workflows/python-dependency-layers.yml",
        ".github/workflows/docker-build.yml",
        ".github/PULL_REQUEST_TEMPLATE/agent_canon.md",
        ".github/scripts/checkout_agent_canon_submodule.sh",
        "scripts/start_repository.sh",
        "scripts/init_from_template.sh",
        "scripts/check-python-dependency-layers.py",
        "scripts/build-release-manifest.py",
        "documents/template-bootstrap.md",
        "documents/template-github-remote.md",
        "documents/repository-audit-checklist.md",
        "documents/dependency-layers.md",
        "documents/server-host-contract.md",
        "documents/remote-execution-repo-contract.md",
        "documents/linux-wsl-host-requirements.md",
    ]:
        remove(path)

    keep_test_python = {
        "tests/tools/__init__.py",
        "tests/tools/test_release_metadata.py",
    }
    for path in sorted(tracked_paths()):
        if path.startswith("tests/tools/") and path.endswith(".py") and path not in keep_test_python:
            remove(path)
        elif path.startswith("tests/") and path.endswith(".py") and path not in keep_test_python:
            remove(path)

    keep_scripts = {
        "scripts/README.md",
        "scripts/check-ptymark-renderers.sh",
        "scripts/check-ptymark-runtime-dependencies.mjs",
        "scripts/check-release-metadata.py",
        "scripts/install-managed-bundle.ps1",
        "scripts/install-managed-bundle.sh",
        "scripts/install.cmd",
        "scripts/install.ps1",
        "scripts/install.sh",
        "scripts/installer.cmd",
        "scripts/installer.ps1",
        "scripts/installer.sh",
        "scripts/package-release.ps1",
        "scripts/package-release.sh",
        "scripts/alpha4_fixup.py",
        "scripts/remove_agent_canon.py",
    }
    for path in sorted(tracked_paths()):
        if path.startswith("scripts/") and path not in keep_scripts:
            remove(path)

    keep_docker = {
        "docker/README.md",
        "docker/ptymark.Dockerfile",
        "docker/ptymark-compose.yaml",
        "docker/ptymark-versions.env",
    }
    for path in sorted(tracked_paths()):
        if path.startswith("docker/") and path not in keep_docker:
            remove(path)


def rewrite_product_files() -> None:
    write(
        ".gitignore",
        """# Rust and local build output
/target/
/dist/

# Managed renderer dependencies
/renderers/node_modules/

# Local caches, reports, and secrets
/.state/
/.cache/
/reports/
*.log
.env
.env.*
!.env.example

# Editor and operating-system noise
.DS_Store
.vscode/
""",
    )

    write(
        ".dockerignore",
        """.git
target
dist
renderers/node_modules
reports
.state
.cache
.env
.env.*
""",
    )

    write(
        "AGENTS.md",
        """# Ptymark contributor instructions

Ptymark is a Rust terminal pre-display renderer. Keep changes product-focused and preserve these contracts:

- terminal controls, progress redraws, alternate-screen traffic, and child argv stay byte-exact;
- user-authored TOML contains portable intent, while resolved paths and hard safety limits remain internal state;
- renderer failure restores exact source unless strict mode was explicitly selected;
- external commands are invoked as typed argv and never through shell interpolation;
- GitHub releases are source-only and must not upload project-built executables.

Before opening or updating a pull request, run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Use `make ptymark-check` for the canonical Docker acceptance path when installer, renderer, shell, or container behavior changes.
""",
    )

    write(
        "Makefile",
        """# Product-only developer entrypoints. Detailed Docker commands live in ptymark.mk.
include ptymark.mk

.PHONY: all build check ci ci-full fmt lint test runtime-dependencies verify-catalog dev clean release-metadata

all: check

build:
\tcargo build --locked

fmt:
\tcargo fmt --all -- --check

lint:
\tcargo clippy --locked --all-targets -- -D warnings

test:
\tcargo test --locked --all-targets

check: fmt lint test

ci: check

ci-full: ptymark-check

runtime-dependencies: ptymark-runtime-dependencies

verify-catalog: ptymark-verify-catalog

dev: ptymark-dev

clean: ptymark-clean

release-metadata:
\tpython3 scripts/check-release-metadata.py
""",
    )
    write("GNUmakefile", "include Makefile")

    write(
        "docker/README.md",
        """# Ptymark development container

The canonical product container is intentionally limited to the Rust core, managed renderers, shell tooling, Chromium, and Lua used by the WezTerm smoke test.

```bash
make ptymark-build
make ptymark-check
make ptymark-dev
make ptymark-clean
```

Files:

- `ptymark.Dockerfile`: pinned validation image;
- `ptymark-compose.yaml`: local and CI compose entrypoint;
- `ptymark-versions.env`: toolchain image/version ownership.

The container is verification infrastructure, not a release artifact. GitHub releases remain source-only.
""",
    )

    write(
        "scripts/README.md",
        """# Ptymark scripts

This directory contains product-owned installation, renderer validation, local package smoke, and release-metadata utilities.

- `installer.sh`, `installer.ps1`, `installer.cmd`: canonical source-install frontends;
- `install-managed-bundle.*`: isolated managed-renderer installation;
- `check-ptymark-renderers.sh`: selected renderer acceptance;
- `check-ptymark-runtime-dependencies.mjs`: version and dependency alignment;
- `package-release.*`: developer/CI-only local package smoke; outputs are discarded;
- `check-release-metadata.py`: source-only release contract validation.

Scripts must not edit shell profiles automatically, create a global Node installation, or publish executable release assets.
""",
    )

    write(
        "documents/README.md",
        """<!--
@dependency-start
contract design
responsibility Provides the task-oriented documentation map for product-owned Ptymark contracts.
upstream design ../README.md product entrypoint
downstream design ./ptymark-design.md architecture contract
downstream design ./ptymark-installer.md installation contract
downstream design ../verification/README.md verification policy
@dependency-end
-->

# Documentation map

| Goal | Start here | Continue with |
| --- | --- | --- |
| Build and install from source | [Root README](../README.md) | [Installer design](./ptymark-installer.md) |
| Verify or troubleshoot an installation | [Troubleshooting](./troubleshooting.md) | [Verification catalog](../verification/README.md) |
| Understand terminal and rendering safety | [Ptymark design](./ptymark-design.md) | [Interactive sessions](./interactive-session.md) |
| Render structured mathematics | [OpenMath input](./openmath.md) | [Runnable example](../examples/openmath.md) |
| Filter non-interactive command output | [Filtered command execution](./filtered-command.md) | [Architecture](./ptymark-design.md) |
| Review shell coexistence | [Shell compatibility](./shell-plugin-compatibility.md) | [Verification catalog](../verification/README.md) |
| Change dependencies | [Runtime dependencies](./ptymark-runtime-dependencies.md) | [Canonical Docker environment](../docker/README.md) |
| Prepare a release | [Release contract](./release.md) | [Release metadata check](../scripts/check-release-metadata.py) |

## Product contracts

- [`ptymark-design.md`](./ptymark-design.md): architecture, terminal invariants, rendering boundaries, and extension rules.
- [`ptymark-installer.md`](./ptymark-installer.md): source installation, managed renderer isolation, and state ownership.
- [`interactive-session.md`](./interactive-session.md): Unix PTY and Windows ConPTY behavior.
- [`filtered-command.md`](./filtered-command.md): pipe-oriented child execution.
- [`openmath.md`](./openmath.md): bounded OpenMath parsing and conversion.
- [`troubleshooting.md`](./troubleshooting.md): diagnostics, recovery, and support reports.
- [`shell-plugin-compatibility.md`](./shell-plugin-compatibility.md): reviewed shell/plugin coexistence.
- [`ptymark-runtime-dependencies.md`](./ptymark-runtime-dependencies.md): Rust and managed-renderer dependency ownership.
- [`release.md`](./release.md): source-only publication and rollback.
- [`alpha4-design.md`](./alpha4-design.md): Alpha.4 configuration and internal-policy separation.

All documents in this directory are owned by Ptymark. Shared repository-template or external agent-runtime policy is not vendored into the product repository.
""",
    )

    write(
        ".github/dependabot.yml",
        """version: 2
updates:
  - package-ecosystem: cargo
    directory: "/"
    schedule:
      interval: weekly
      day: monday
      time: "03:00"
      timezone: Asia/Tokyo
    open-pull-requests-limit: 5
    groups:
      rust-dependencies:
        patterns: ["*"]

  - package-ecosystem: npm
    directory: "/renderers"
    schedule:
      interval: weekly
      day: monday
      time: "03:15"
      timezone: Asia/Tokyo
    open-pull-requests-limit: 5
    groups:
      renderer-dependencies:
        patterns: ["*"]

  - package-ecosystem: docker
    directory: "/docker"
    schedule:
      interval: weekly
      day: monday
      time: "03:30"
      timezone: Asia/Tokyo
    open-pull-requests-limit: 5

  - package-ecosystem: github-actions
    directory: "/"
    schedule:
      interval: weekly
      day: monday
      time: "03:45"
      timezone: Asia/Tokyo
    open-pull-requests-limit: 5
    groups:
      github-actions:
        patterns: ["*"]
""",
    )

    write(
        ".github/workflows/ci.yml",
        """# Lightweight repository wiring check. Product behavior is owned by ptymark-ci.yml.
name: CI

on:
  pull_request:
    branches: [main]
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read

concurrency:
  group: repository-ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  repository-ci:
    name: Repository CI
    runs-on: ubuntu-24.04
    timeout-minutes: 10
    steps:
      - name: Check out repository
        uses: actions/checkout@9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0 # v7
        with:
          persist-credentials: false
          submodules: false
      - name: Install pinned Rust toolchain
        run: |
          rustup toolchain install 1.97.0 --profile minimal
          rustup override set 1.97.0
      - name: Validate repository wiring
        run: |
          git diff --check
          cargo metadata --locked --no-deps --format-version 1 >/dev/null
          python3 scripts/check-release-metadata.py
""",
    )


def remove_yaml_step(path: str, step_name: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    lines = text.splitlines()
    output: list[str] = []
    index = 0
    marker = f"      - name: {step_name}"
    while index < len(lines):
        if lines[index] == marker:
            index += 1
            while index < len(lines) and not lines[index].startswith("      - name:"):
                index += 1
            continue
        output.append(lines[index])
        index += 1
    target.write_text("\n".join(output).rstrip() + "\n", encoding="utf-8")


def rewrite_workflows() -> None:
    remove_yaml_step(
        ".github/workflows/ptymark-ci.yml",
        "Check out AgentCanon submodule for the canonical image",
    )
    remove_yaml_step(
        ".github/workflows/ptymark-release.yml",
        "Check out AgentCanon submodule",
    )
    remove_yaml_step(
        ".github/workflows/ptymark-release-metadata.yml",
        "Check out AgentCanon submodule",
    )

    metadata = ROOT / ".github/workflows/ptymark-release-metadata.yml"
    text = metadata.read_text(encoding="utf-8")
    text = text.replace("      - scripts/build-release-manifest.py\n", "")
    metadata.write_text(text, encoding="utf-8")

    ptymark_ci = ROOT / ".github/workflows/ptymark-ci.yml"
    text = ptymark_ci.read_text(encoding="utf-8")
    needle = """            while IFS= read -r path; do
              case \"$path\" in
                src/config.rs|src/install.rs|src/managed_launcher.rs|src/engine.rs|scripts/*|distribution/*|compat/*|renderers/*|tests/install*|tests/managed*|tests/windows*|tests/shell_profile*|.github/workflows/ptymark-ci.yml)
"""
    replacement = """            while IFS= read -r path; do
              case \"$path\" in
                Cargo.toml|Cargo.lock|CHANGELOG.md|release-notes/*|.github/workflows/ptymark-release*.yml)
                  release=true
                  ;;
              esac
              case \"$path\" in
                src/config.rs|src/install.rs|src/managed_launcher.rs|src/engine.rs|scripts/*|distribution/*|compat/*|renderers/*|tests/install*|tests/managed*|tests/windows*|tests/shell_profile*|.github/workflows/ptymark-ci.yml)
"""
    if needle not in text:
        raise RuntimeError("cannot locate ptymark CI change-classification block")
    text = text.replace(needle, replacement, 1)
    text = text.replace(
        """          \"${compose[@]}\" run --rm --no-TTY dev node --check renderers/managed/mathjax-cli.mjs
          \"${compose[@]}\" run --rm --no-TTY dev node --check renderers/managed/ansi-presenter.mjs
""",
        """          \"${compose[@]}\" run --rm --no-TTY dev node --check renderers/managed/mathjax-cli.mjs
          \"${compose[@]}\" run --rm --no-TTY dev node --check renderers/managed/ansi-presenter.mjs
          \"${compose[@]}\" run --rm --no-TTY dev lua5.4 tests/plugin_smoke.lua
""",
        1,
    )
    text = text.replace(
        "bash -lc 'cargo build --locked && PTYMARK_TEST_BROWSER=/usr/bin/chromium PTYMARK_BROWSER_NO_SANDBOX=1 bash tests/managed_renderer_smoke.sh'",
        "bash -lc 'cargo build --locked && PTYMARK_TEST_BROWSER=/usr/bin/chromium PTYMARK_BROWSER_NO_SANDBOX=1 bash tests/managed_renderer_smoke.sh && bash scripts/check-ptymark-renderers.sh'",
        1,
    )
    ptymark_ci.write_text(text, encoding="utf-8")

    codeql = ROOT / ".github/workflows/codeql.yml"
    text = codeql.read_text(encoding="utf-8")
    text = text.replace(
        "# responsibility Runs CodeQL analysis for the Rust core, Python tooling, renderer JavaScript, and GitHub Actions.\n",
        "# responsibility Runs CodeQL analysis for the Rust core, renderer JavaScript, and GitHub Actions.\n",
    )
    text = text.replace("# upstream implementation ../../python Python tooling surface\n", "")
    text = text.replace('      - "python/**"\n', "")
    text = text.replace(
        """          - language: python
            build-mode: none
""",
        "",
    )
    codeql.write_text(text, encoding="utf-8")


def rewrite_verification_catalog() -> None:
    manifest = ROOT / "verification/manifest.toml"
    text = manifest.read_text(encoding="utf-8")
    text = text.replace(
        "# upstream implementation ../.github/workflows/ci.yml repository checks\n",
        "",
    )
    text = text.replace(
        """repository_workflow = \".github/workflows/ci.yml\"
docker_workflow = \".github/workflows/docker-build.yml\"
agent_workflow = \".github/workflows/agent-improvement-guide.yml\"
""",
        "repository_workflow = \".github/workflows/ci.yml\"\n",
    )
    marker = '\n[[check]]\nid = "repository.ci"\n'
    if marker not in text:
        raise RuntimeError("verification manifest repository section was not found")
    text = text.split(marker, 1)[0].rstrip() + "\n"
    manifest.write_text(text, encoding="utf-8")

    test = ROOT / "tests/verification_manifest_contract.rs"
    text = test.read_text(encoding="utf-8")
    text = text.replace(
        """    repository_workflow: String,
    docker_workflow: String,
    agent_workflow: String,
""",
        "    repository_workflow: String,\n",
    )
    text = text.replace(
        """    \"package-smoke.windows\",
    \"repository.ci\",
    \"repository.docker-build\",
    \"repository.agent-improvement-guide\",
""",
        "    \"package-smoke.windows\",\n",
    )
    text = text.replace(
        """        &manifest.policy.repository_workflow,
        &manifest.policy.docker_workflow,
        &manifest.policy.agent_workflow,
        &manifest.policy.compatibility_inventory,
""",
        """        &manifest.policy.repository_workflow,
        &manifest.policy.compatibility_inventory,
""",
    )
    text = text.replace(
        """    let allowed_owners = HashSet::from([
        \"ptymark-ci\",
        \"repository-ci\",
        \"docker-build\",
        \"agent-improvement-guide\",
    ]);""",
        '    let allowed_owners = HashSet::from(["ptymark-ci"]);',
    )
    test.write_text(text, encoding="utf-8")

    readme = ROOT / "verification/README.md"
    text = readme.read_text(encoding="utf-8")
    text = text.replace(
        "upstream implementation ../.github/workflows/ci.yml repository evidence\n",
        "",
    )
    text = text.replace(
        "- `.github/workflows/ci.yml`, `docker-build.yml`, and `agent-improvement-guide.yml` remain independent repository gates.\n",
        "- `.github/workflows/ci.yml` performs a lightweight repository-wiring check; all feature evidence is owned by the product workflow.\n",
    )
    text = text.replace(
        "| `repository` | Inherited template, Docker-pack, and AgentCanon-owned gates. |\n",
        "| `repository` | Lightweight repository wiring that is independent of feature behavior. |\n",
    )
    text = text.replace(
        """### Independent repository gates

- `repository.ci`
- `repository.docker-build`
- `repository.agent-improvement-guide`

""",
        "",
    )
    readme.write_text(text, encoding="utf-8")


def assert_no_agent_canon() -> None:
    forbidden: list[str] = []
    for path in sorted(tracked_paths()):
        lowered = path.lower()
        if "agent-canon" in lowered or "agent_canon" in lowered:
            forbidden.append(path)
            continue
        target = ROOT / path
        if target.is_symlink():
            if "agent-canon" in os.readlink(target).lower():
                forbidden.append(path)
            continue
        if not target.is_file():
            continue
        try:
            text = target.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        if "agentcanon" in text.lower() or "agent-canon" in text.lower() or "agent_canon" in text.lower():
            forbidden.append(path)
    if forbidden:
        raise RuntimeError("AgentCanon references remain: " + ", ".join(forbidden))

    gitlinks = run("git", "ls-files", "-s").stdout.splitlines()
    gitlinks = [line for line in gitlinks if line.startswith("160000 ")]
    if gitlinks:
        raise RuntimeError("submodule gitlinks remain: " + "; ".join(gitlinks))


def remove_temporary_cleanup_files() -> None:
    for path in [
        ".github/workflows/alpha4-lockfile.yml",
        "scripts/alpha4_fixup.py",
        "scripts/remove_agent_canon.py",
    ]:
        remove(path)


def main() -> None:
    remove_agent_links()
    remove_template_surfaces()
    rewrite_product_files()
    rewrite_workflows()
    rewrite_verification_catalog()
    remove_temporary_cleanup_files()
    assert_no_agent_canon()
    subprocess.run(["git", "add", "-A"], cwd=ROOT, check=True)


if __name__ == "__main__":
    main()
