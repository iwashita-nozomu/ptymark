# @dependency-start
# contract tool
# responsibility Applies bounded release-candidate metadata and package-smoke repairs.
# upstream design ../../documents/release.md defines release readiness and package evidence.
# downstream implementation ../workflows/ptymark-release-metadata.yml validates and commits the repaired tree.
# @dependency-end
from __future__ import annotations

import runpy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
runpy.run_path(str(ROOT / ".github/scripts/repair_ptymark_dependency_headers.py"))

SPECS: dict[str, tuple[str, str, list[str]]] = {
    ".github/scripts/repair_ptymark_dependency_headers.py": (
        "tool",
        "Repairs ptymark dependency manifests to the pinned AgentCanon grammar.",
        [
            "upstream design ../../vendor/agent-canon/documents/dependency-manifest-design.md manifest DSL",
            "downstream implementation ../workflows/ptymark-release-metadata.yml bounded release repair",
        ],
    ),
    ".github/scripts/apply_release_candidate_fixes.py": (
        "tool",
        "Applies bounded release-candidate metadata and package-smoke repairs.",
        [
            "upstream design ../../documents/release.md release readiness and package evidence",
            "downstream implementation ../workflows/ptymark-release-metadata.yml validates and commits the repaired tree",
        ],
    ),
    ".github/workflows/ptymark-release.yml": (
        "workflow",
        "Builds, verifies, attests, tags, and publishes immutable prerelease assets.",
        [
            "upstream design ../../documents/release.md release invariants and recovery",
            "upstream implementation ../../scripts/check-release-metadata.py source validation",
            "upstream implementation ../../scripts/build-release-manifest.py checksums and metadata",
            "upstream implementation ../../scripts/package-release.sh Unix package assembly",
            "upstream implementation ../../scripts/package-release.ps1 Windows package assembly",
        ],
    ),
    ".github/workflows/ptymark-release-metadata.yml": (
        "workflow",
        "Validates version, packaging, and release metadata on main and pull requests.",
        [
            "upstream design ../../documents/release.md publication contract",
            "upstream implementation ../../scripts/check-release-metadata.py tree validator",
            "upstream implementation ../../tests/tools/test_release_metadata.py metadata tests",
            "downstream implementation ./ptymark-release.yml publication workflow",
        ],
    ),
    "documents/release.md": (
        "design",
        "Defines immutable release publication, asset verification, and recovery behavior.",
        [
            "upstream environment ../Cargo.toml package version",
            "upstream implementation ../scripts/check-release-metadata.py tree validation",
            "upstream implementation ../scripts/build-release-manifest.py release metadata generation",
            "downstream implementation ../.github/workflows/ptymark-release.yml publication orchestration",
        ],
    ),
    "scripts/check-release-metadata.py": (
        "implementation",
        "Validates version, documentation, packaging, and workflow release metadata.",
        [
            "upstream environment ../Cargo.toml package version",
            "upstream design ../documents/release.md release contract",
            "downstream implementation ../.github/workflows/ptymark-release.yml publication gate",
            "downstream implementation ../tests/tools/test_release_metadata.py metadata tests",
        ],
    ),
    "scripts/build-release-manifest.py": (
        "implementation",
        "Verifies archives and generates checksums, notes, and machine-readable metadata.",
        [
            "upstream environment ../Cargo.toml package version",
            "upstream environment ../renderers/managed-bundle.env managed compatibility versions",
            "upstream design ../documents/release.md asset contract",
            "downstream implementation ../.github/workflows/ptymark-release.yml publication orchestration",
            "downstream implementation ../tests/tools/test_release_metadata.py manifest tests",
        ],
    ),
    "scripts/package-release.ps1": (
        "implementation",
        "Builds one versioned Windows release archive and checksum.",
        [
            "upstream design ../documents/release.md release asset contract",
            "upstream environment ../Cargo.toml package version",
            "downstream environment ../.github/workflows/ptymark-release.yml publication workflow",
        ],
    ),
}


def render(path: Path, contract: str, responsibility: str, edges: list[str]) -> list[str]:
    body = [
        "@dependency-start",
        f"contract {contract}",
        f"responsibility {responsibility}",
        *edges,
        "@dependency-end",
    ]
    if path.suffix == ".md":
        return ["<!--", *body, "-->"]
    return [f"# {line}" for line in body]


def rewrite(path: Path, spec: tuple[str, str, list[str]]) -> None:
    lines = path.read_text(encoding="utf-8").splitlines()
    start = next((index for index, line in enumerate(lines) if "@dependency-start" in line), None)
    if start is not None:
        end = next(index for index in range(start, len(lines)) if "@dependency-end" in lines[index])
        if start and lines[start - 1].strip() == "<!--":
            start -= 1
        if end + 1 < len(lines) and lines[end + 1].strip() == "-->":
            end += 1
        del lines[start : end + 1]
    insert = 1 if lines and (lines[0].startswith("#!") or (path.suffix == ".md" and lines[0].startswith("# "))) else 0
    rest = lines[insert:]
    while rest and not rest[0].strip():
        rest.pop(0)
    output = lines[:insert] + ([""] if insert else []) + render(path, *spec) + [""] + rest
    path.write_text("\n".join(output).rstrip() + "\n", encoding="utf-8")


for relative, spec in SPECS.items():
    rewrite(ROOT / relative, spec)

package = ROOT / "scripts/package-release.sh"
text = package.read_text(encoding="utf-8")
old = """interactive_script=$(cat <<'EOF_INTERACTIVE_SCRIPT'
printf '$$\\nE = mc^2\\n$$\\n'"""
new = """# macOS PTYs can echo a non-TTY stdin EOF as control bytes on the current line.
# Start semantic output after a newline so the safety gate can preserve that line raw.
interactive_script=$(cat <<'EOF_INTERACTIVE_SCRIPT'
printf '\\n$$\\nE = mc^2\\n$$\\n'"""
if old in text:
    package.write_text(text.replace(old, new), encoding="utf-8")
elif new not in text:
    raise SystemExit("release package interactive smoke fixture was not found")

design = ROOT / "documents/ptymark-design.md"
text = design.read_text(encoding="utf-8")
old_math = "- ``$$ ... $$``;"
new_math = "- a line-bounded block-math fence delimited by two dollar-sign lines;"
if old_math in text:
    design.write_text(text.replace(old_math, new_math), encoding="utf-8")
elif new_math not in text:
    raise SystemExit("expected block-math description was not found")
