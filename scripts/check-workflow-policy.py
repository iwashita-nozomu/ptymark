#!/usr/bin/env python3

# @dependency-start
# contract implementation
# responsibility Validates immutable GitHub Action references, merge-result CodeQL identity, stable PR gates, and prerelease workflow boundaries.
# upstream environment ../.github/workflows GitHub Actions definitions
# upstream design ../documents/release.md source-only prerelease contract
# downstream environment ../.github/workflows/ci.yml repository wiring gate
# downstream implementation ../tests/tools/test_workflow_policy.py policy regression tests
# @dependency-end

"""Validate GitHub Actions supply-chain and prerelease policy without dependencies."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

USES_PATTERN = re.compile(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)")
FULL_COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")


def _read(path: Path, failures: list[str]) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as error:
        failures.append(f"cannot read {path}: {error}")
        return ""


def _validate_action_pins(root: Path, failures: list[str]) -> None:
    workflow_dir = root / ".github" / "workflows"
    paths = sorted(workflow_dir.glob("*.yml")) + sorted(workflow_dir.glob("*.yaml"))
    if not paths:
        failures.append("no GitHub Actions workflows were found")
        return

    for path in paths:
        relative = path.relative_to(root)
        content = _read(path, failures)
        for line_number, line in enumerate(content.splitlines(), start=1):
            match = USES_PATTERN.match(line)
            if match is None:
                continue
            specification = match.group(1).strip("'\"")
            if specification.startswith("./") or specification.startswith("docker://"):
                continue
            if "@" not in specification:
                failures.append(
                    f"{relative}:{line_number}: remote action has no immutable ref: {specification}"
                )
                continue
            _, ref = specification.rsplit("@", 1)
            if not FULL_COMMIT_PATTERN.fullmatch(ref):
                failures.append(
                    f"{relative}:{line_number}: remote action must use a full 40-character commit SHA: {specification}"
                )


def _validate_codeql(root: Path, failures: list[str]) -> None:
    path = root / ".github" / "workflows" / "codeql.yml"
    content = _read(path, failures)
    if not content:
        return

    required_markers = (
        "pull_request:",
        "EXPECTED_SHA: ${{ github.sha }}",
        "Analyze the GitHub pull-request merge result",
        "category: \"/language:${{ matrix.language }}\"",
        "security-events: write",
    )
    for marker in required_markers:
        if marker not in content:
            failures.append(f"CodeQL workflow is missing required merge-result marker: {marker}")

    forbidden_markers = (
        "github.event.pull_request.head.sha",
        "github.event.pull_request.head.ref",
        "refs/pull/{0}/head",
        "refs/heads/{0}",
        "Analyze and register the stable PR head",
        "Analyze and register the stable source ref",
    )
    for marker in forbidden_markers:
        if marker in content:
            failures.append(
                f"CodeQL workflow must analyze GitHub's protected event commit instead of overriding the PR source: {marker}"
            )

    checkout_section = content.split("actions/checkout@", maxsplit=1)
    if len(checkout_section) != 2:
        failures.append("CodeQL workflow has no checkout action")
    else:
        before_init = checkout_section[1].split("github/codeql-action/init@", maxsplit=1)[0]
        for marker in ("\n          repository: ${{", "\n          ref: ${{"):
            if marker in before_init:
                failures.append(
                    "CodeQL checkout must retain the GitHub event commit; repository/ref overrides are forbidden"
                )

    analyze_section = content.split("github/codeql-action/analyze@", maxsplit=1)
    if len(analyze_section) != 2:
        failures.append("CodeQL workflow has no analyze action")
        return

    action_inputs = analyze_section[1]
    for marker in ("\n          ref: ${{", "\n          sha: ${{"):
        if marker in action_inputs:
            failures.append(
                "CodeQL analyze must not override ref/sha; GitHub must register the protected event commit"
            )


def _validate_product_gate(root: Path, failures: list[str]) -> None:
    path = root / ".github" / "workflows" / "ptymark-ci.yml"
    content = _read(path, failures)
    if not content:
        return

    for marker in ("name: Ptymark PR Gate", "if: always()", "pull_request:"):
        if marker not in content:
            failures.append(f"product CI is missing required PR gate marker: {marker}")

    try:
        pull_request_section = content.split("\n  pull_request:", maxsplit=1)[1].split(
            "\n  push:", maxsplit=1
        )[0]
    except IndexError:
        failures.append("product CI must declare pull_request before push")
        return

    for marker in ("paths:", "paths-ignore:"):
        if marker in pull_request_section:
            failures.append(
                "product CI pull_request trigger must not use path filters; the stable PR gate must always be emitted"
            )


def _validate_prerelease(root: Path, failures: list[str]) -> None:
    path = root / ".github" / "workflows" / "ptymark-release.yml"
    content = _read(path, failures)
    if not content:
        return

    required_markers = (
        "--prerelease",
        "source prerelease workflow accepts only alpha, beta, or rc versions",
        "release/v*",
        "source-only",
    )
    for marker in required_markers:
        if marker not in content:
            failures.append(f"source prerelease workflow is missing required marker: {marker}")

    for marker in ("--latest", "make_latest=legacy", "make_latest=true"):
        if marker in content:
            failures.append(f"source prerelease workflow contains a stable-release marker: {marker}")


def validate(root: Path) -> list[str]:
    failures: list[str] = []
    _validate_action_pins(root, failures)
    _validate_codeql(root, failures)
    _validate_product_gate(root, failures)
    _validate_prerelease(root, failures)
    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    arguments = parser.parse_args(argv)
    root = arguments.root.resolve()
    failures = validate(root)
    if failures:
        for failure in failures:
            print(f"workflow policy error: {failure}", file=sys.stderr)
        return 1
    print("workflow security policy ok")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
