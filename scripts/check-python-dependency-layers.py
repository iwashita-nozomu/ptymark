#!/usr/bin/env python3
# @dependency-start
# contract tool
# responsibility Validates and regenerates repository Python runtime and verification dependency layers.
# upstream environment ../docker/requirements-runtime.txt canonical Python runtime dependencies
# upstream environment ../docker/requirements-dev.txt canonical Python verification dependencies
# upstream environment ../docker/requirements.txt generated compatibility aggregate
# upstream configuration ../pyproject.toml package-local Python dependency metadata
# upstream configuration ../.github/dependabot.yml dependency update routing
# downstream workflow ../.github/workflows/python-dependency-layers.yml runs this contract
# downstream design ../documents/dependency-layers.md dependency ownership and upgrade rules
# @dependency-end
"""Validate and regenerate the repository Python dependency layers."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME_PATH = ROOT / "docker" / "requirements-runtime.txt"
VERIFICATION_PATH = ROOT / "docker" / "requirements-dev.txt"
AGGREGATE_PATH = ROOT / "docker" / "requirements.txt"
PYPROJECT_PATH = ROOT / "pyproject.toml"
INSTALLER_PATH = ROOT / "docker" / "install_python_dependencies.sh"
CI_WORKFLOW_PATH = ROOT / ".github" / "workflows" / "ci.yml"
LAYER_WORKFLOW_PATH = ROOT / ".github" / "workflows" / "python-dependency-layers.yml"
DEPENDABOT_PATH = ROOT / ".github" / "dependabot.yml"

_PACKAGE_PATTERN = re.compile(r"^([A-Za-z0-9][A-Za-z0-9._-]*)(?:\[[^]]+\])?")


def requirement_specs(path: Path) -> list[str]:
    """Return non-comment requirement specifications from a leaf file."""
    specs: list[str] = []
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("-"):
            raise ValueError(
                f"{path.relative_to(ROOT)}:{line_number}: leaf files cannot include pip options"
            )
        if _PACKAGE_PATTERN.match(line) is None:
            raise ValueError(
                f"{path.relative_to(ROOT)}:{line_number}: unsupported requirement: {line}"
            )
        specs.append(line)
    return specs


def normalize_package_name(spec: str) -> str:
    """Return the normalized package name from one requirement specification."""
    match = _PACKAGE_PATTERN.match(spec)
    if match is None:
        raise ValueError(f"unsupported requirement: {spec}")
    return re.sub(r"[-_.]+", "-", match.group(1)).lower()


def package_names(specs: list[str]) -> list[str]:
    """Return normalized package names in declaration order."""
    return [normalize_package_name(spec) for spec in specs]


def generated_aggregate(runtime_specs: list[str], verification_specs: list[str]) -> str:
    """Render the compatibility aggregate from the two canonical leaf files."""
    runtime = "\n".join(runtime_specs)
    verification = "\n".join(verification_specs)
    return f"""# @dependency-start
# contract environment
# responsibility Provides the generated full Python development aggregate for compatibility installers.
# upstream environment requirements-runtime.txt canonical workload runtime dependencies
# upstream environment requirements-dev.txt canonical verification-only dependencies
# upstream tool ../scripts/check-python-dependency-layers.py validates and regenerates this aggregate
# downstream implementation install_python_dependencies.sh default verification profile
# @dependency-end
# Generated file. Edit requirements-runtime.txt or requirements-dev.txt, then run:
# python3 scripts/check-python-dependency-layers.py --write

# Runtime dependencies
{runtime}

# Verification dependencies
{verification}
"""


def optional_dependency_specs(pyproject: dict[str, object], name: str) -> list[str]:
    """Read one optional-dependency list from parsed project metadata."""
    project = pyproject.get("project")
    if not isinstance(project, dict):
        raise ValueError("pyproject.toml: missing [project]")
    optional = project.get("optional-dependencies")
    if not isinstance(optional, dict):
        raise ValueError("pyproject.toml: missing [project.optional-dependencies]")
    value = optional.get(name)
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(
            f"pyproject.toml: optional dependency group {name!r} must be a string list"
        )
    return [item for item in value if isinstance(item, str)]


def project_runtime_specs(pyproject: dict[str, object]) -> list[str]:
    """Read direct Python project runtime dependencies."""
    project = pyproject.get("project")
    if not isinstance(project, dict):
        raise ValueError("pyproject.toml: missing [project]")
    value = project.get("dependencies", [])
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError("pyproject.toml: project.dependencies must be a string list")
    return [item for item in value if isinstance(item, str)]


def duplicate_names(names: list[str]) -> list[str]:
    """Return sorted duplicate package names."""
    seen: set[str] = set()
    duplicates: set[str] = set()
    for name in names:
        if name in seen:
            duplicates.add(name)
        seen.add(name)
    return sorted(duplicates)


def bounded_section(text: str, start_marker: str, end_marker: str) -> str | None:
    """Return text between two required routing markers."""
    start = text.find(start_marker)
    if start < 0:
        return None
    end = text.find(end_marker, start + len(start_marker))
    if end < 0:
        return None
    return text[start:end]


def validate_dependabot_groups(
    dependabot: str,
    runtime_names: list[str],
    verification_names: list[str],
    issues: list[str],
) -> None:
    """Validate that every container package is routed to its ownership group."""
    runtime_group = bounded_section(
        dependabot,
        "      python-container-runtime:\n",
        "      python-container-verification:\n",
    )
    verification_group = bounded_section(
        dependabot,
        "      python-container-verification:\n",
        "\n  - package-ecosystem: docker\n",
    )
    if runtime_group is None:
        issues.append(".github/dependabot.yml: cannot isolate python-container-runtime group")
    else:
        for name in runtime_names:
            if f'          - "{name}"' not in runtime_group:
                issues.append(
                    f".github/dependabot.yml: runtime group does not route package {name}"
                )
    if verification_group is None:
        issues.append(
            ".github/dependabot.yml: cannot isolate python-container-verification group"
        )
    else:
        for name in verification_names:
            if f'          - "{name}"' not in verification_group:
                issues.append(
                    f".github/dependabot.yml: verification group does not route package {name}"
                )


def validate_text_contracts(
    runtime_names: list[str], verification_names: list[str], issues: list[str]
) -> None:
    """Validate installer, CI, focused workflow, and update routing contracts."""
    installer = INSTALLER_PATH.read_text(encoding="utf-8")
    for required in (
        "--profile",
        "--print-requirements",
        "requirements-runtime.txt",
        "requirements-dev.txt",
        "requirements.txt",
        "python-requirements-${profile}.sha256",
    ):
        if required not in installer:
            issues.append(f"docker/install_python_dependencies.sh: missing {required!r}")

    workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
    for required in (
        "docker/requirements.txt",
        "docker/requirements-runtime.txt",
        "docker/requirements-dev.txt",
    ):
        if required not in workflow:
            issues.append(f".github/workflows/ci.yml: cache does not track {required}")
    install_lines = [
        line.strip()
        for line in workflow.splitlines()
        if "docker/install_python_dependencies.sh" in line
    ]
    if not install_lines:
        issues.append(".github/workflows/ci.yml: no Python dependency installer invocation found")
    for line in install_lines:
        if "--profile verification" not in line:
            issues.append(
                ".github/workflows/ci.yml: CI installer must select verification profile: "
                + line
            )

    layer_workflow = LAYER_WORKFLOW_PATH.read_text(encoding="utf-8")
    for required in (
        "python3 scripts/check-python-dependency-layers.py",
        "--profile runtime --print-requirements",
        "--profile verification --print-requirements",
    ):
        if required not in layer_workflow:
            issues.append(
                f".github/workflows/python-dependency-layers.yml: missing {required!r}"
            )

    dependabot = DEPENDABOT_PATH.read_text(encoding="utf-8")
    for required in (
        "python-project-runtime",
        "python-project-verification",
        "python-container-runtime",
        "python-container-verification",
        '"docker/**"',
    ):
        if required not in dependabot:
            issues.append(
                f".github/dependabot.yml: missing dependency routing marker {required!r}"
            )
    validate_dependabot_groups(dependabot, runtime_names, verification_names, issues)


def validate(write: bool) -> int:
    """Validate all dependency-layer invariants and optionally rewrite the aggregate."""
    issues: list[str] = []
    try:
        runtime_specs = requirement_specs(RUNTIME_PATH)
        verification_specs = requirement_specs(VERIFICATION_PATH)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1

    expected_aggregate = generated_aggregate(runtime_specs, verification_specs)
    if write:
        AGGREGATE_PATH.write_text(expected_aggregate, encoding="utf-8")

    actual_aggregate = AGGREGATE_PATH.read_text(encoding="utf-8")
    if actual_aggregate != expected_aggregate:
        issues.append(
            "docker/requirements.txt is stale; run "
            "python3 scripts/check-python-dependency-layers.py --write"
        )

    runtime_names = package_names(runtime_specs)
    verification_names = package_names(verification_specs)
    runtime_duplicates = duplicate_names(runtime_names)
    verification_duplicates = duplicate_names(verification_names)
    if runtime_duplicates:
        issues.append(f"runtime requirements contain duplicates: {', '.join(runtime_duplicates)}")
    if verification_duplicates:
        issues.append(
            f"verification requirements contain duplicates: {', '.join(verification_duplicates)}"
        )
    overlap = sorted(set(runtime_names) & set(verification_names))
    if overlap:
        issues.append(f"runtime and verification layers overlap: {', '.join(overlap)}")

    with PYPROJECT_PATH.open("rb") as handle:
        pyproject = tomllib.load(handle)
    project_runtime = package_names(project_runtime_specs(pyproject))
    project_verification_specs = optional_dependency_specs(pyproject, "verification")
    project_dev_specs = optional_dependency_specs(pyproject, "dev")
    if project_verification_specs != project_dev_specs:
        issues.append("pyproject.toml: dev must remain an exact compatibility alias of verification")
    project_verification = package_names(project_verification_specs)
    missing_runtime = sorted(set(project_runtime) - set(runtime_names))
    if missing_runtime:
        issues.append(
            "pyproject.toml runtime dependencies missing from requirements-runtime.txt: "
            + ", ".join(missing_runtime)
        )
    missing_verification = sorted(set(project_verification) - set(verification_names))
    if missing_verification:
        issues.append(
            "pyproject.toml verification dependencies missing from requirements-dev.txt: "
            + ", ".join(missing_verification)
        )

    validate_text_contracts(runtime_names, verification_names, issues)

    if issues:
        for issue in issues:
            print(f"dependency_layer_error={issue}", file=sys.stderr)
        return 1

    print(f"PYTHON_RUNTIME_DEPENDENCIES={len(runtime_specs)}")
    print(f"PYTHON_VERIFICATION_DEPENDENCIES={len(verification_specs)}")
    print("PYTHON_DEPENDENCY_LAYERS=pass")
    return 0


def main() -> int:
    """Parse command-line arguments and run the dependency-layer contract."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--write",
        action="store_true",
        help="regenerate docker/requirements.txt from the canonical runtime and dev files",
    )
    args = parser.parse_args()
    return validate(write=args.write)


if __name__ == "__main__":
    raise SystemExit(main())
