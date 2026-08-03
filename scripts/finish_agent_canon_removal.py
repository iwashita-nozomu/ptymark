"""Normalize product documents and their contracts before removing inherited surfaces."""

from __future__ import annotations

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace(path: str, old: str, new: str) -> None:
    target = ROOT / path
    source = target.read_text(encoding="utf-8")
    if old not in source:
        if new in source:
            return
        raise RuntimeError(f"expected fragment not found in {path}")
    target.write_text(source.replace(old, new, 1), encoding="utf-8")


def main() -> None:
    replace(
        "README.md",
        "The complete reader map, including maintainer ownership and shared AgentCanon references, is [`documents/README.md`](documents/README.md).",
        "The complete product documentation map is [`documents/README.md`](documents/README.md).",
    )
    replace(
        "documents/ptymark-runtime-dependencies.md",
        "- the generic AgentCanon/Python/Jupyter repository environment;",
        "- generic Python/Jupyter repository-template environments that are unrelated to the shipped product;",
    )
    replace(
        "tests/documentation_contract.rs",
        """#[test]
fn document_map_starts_with_user_tasks_before_ownership_policy() {
    let tasks = DOCUMENT_MAP.find("## Start by task").expect("task map");
    let ownership = DOCUMENT_MAP
        .find("## Document ownership")
        .expect("ownership section");
    assert!(tasks < ownership);
    for required in [
        "./openmath.md",
        "./interactive-session.md",
        "./filtered-command.md",
        "./troubleshooting.md",
        "../verification/README.md",
    ] {
        assert!(
            DOCUMENT_MAP.contains(required),
            "documentation map is missing `{required}`"
        );
    }
}""",
        """#[test]
fn document_map_routes_user_tasks_to_product_owned_contracts() {
    let tasks = DOCUMENT_MAP
        .find("| Goal | Start here | Continue with |")
        .expect("task map");
    let contracts = DOCUMENT_MAP
        .find("## Product contracts")
        .expect("product contract section");
    assert!(tasks < contracts);
    for required in [
        "./openmath.md",
        "./interactive-session.md",
        "./filtered-command.md",
        "./troubleshooting.md",
        "../verification/README.md",
    ] {
        assert!(
            DOCUMENT_MAP.contains(required),
            "documentation map is missing `{required}`"
        );
    }
}""",
    )

    (ROOT / "documents/licensing-policy.md").write_text(
        """# Licensing Policy

<!--
@dependency-start
contract policy
responsibility Documents Ptymark source and third-party licensing ownership.
upstream design ../README.md product ownership overview
upstream design ../LICENSE repository license text
downstream implementation ../Cargo.toml package license metadata
@dependency-end
-->

Ptymark source code and product-owned repository content are licensed under the
Apache License 2.0, as declared by the root `LICENSE` and `Cargo.toml`.

Third-party Rust and managed-renderer dependencies retain their own licenses.
Their inclusion in a source checkout or isolated managed bundle does not change
those upstream terms. Dependency upgrades must keep license compatibility under
review together with security, MSRV, source-only distribution, and lockfile
changes.

Local developer tools, operating-system packages, browsers, and container images
used for verification are not redistributed as Ptymark release assets. GitHub
releases remain source-only and contain no project-uploaded executables.

When the project license or packaging metadata changes, update the following in
the same reviewed change:

- root `LICENSE`;
- `Cargo.toml` package license metadata;
- README and release documentation;
- downstream package metadata, when an independently maintained package exists.
""",
        encoding="utf-8",
    )

    vendor_readme = ROOT / "vendor/README.md"
    if vendor_readme.exists():
        subprocess.run(
            ["git", "rm", "-f", "--", "vendor/README.md"],
            cwd=ROOT,
            check=True,
        )


if __name__ == "__main__":
    main()
