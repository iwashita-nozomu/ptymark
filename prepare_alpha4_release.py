"""Prepare the v0.1.0-alpha.4 source-only release tree."""

from __future__ import annotations

from pathlib import Path

ROOT = Path.cwd()
VERSION = "0.1.0-alpha.4"
DATE = "2026-08-03"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, text: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8")


def replace_required(path: str, old: str, new: str) -> None:
    text = read(path)
    if old not in text:
        if new in text:
            return
        raise RuntimeError(f"expected release fragment was not found in {path}: {old!r}")
    write(path, text.replace(old, new, 1))


def replace_section(path: str, heading: str, next_heading: str, body: str) -> None:
    text = read(path)
    start = text.find(f"{heading}\n")
    if start < 0:
        raise RuntimeError(f"missing heading {heading!r} in {path}")
    end = text.find(f"{next_heading}\n", start + len(heading))
    if end < 0:
        raise RuntimeError(f"missing next heading {next_heading!r} in {path}")
    replacement = f"{heading}\n\n{body.strip()}\n\n"
    write(path, text[:start] + replacement + text[end:])


def prepare_cargo() -> None:
    replace_required(
        "Cargo.toml",
        'version = "0.1.0-alpha.3"',
        f'version = "{VERSION}"',
    )


def prepare_changelog() -> None:
    text = read("CHANGELOG.md")
    marker = "## [Unreleased]\n\n"
    start = text.find(marker)
    if start < 0:
        raise RuntimeError("CHANGELOG.md has no Unreleased section")
    next_release = text.find("## [0.1.0-alpha.3]", start)
    if next_release < 0:
        raise RuntimeError("CHANGELOG.md has no alpha.3 section")
    section = f"""## [Unreleased]

No user-visible changes are currently queued after `{VERSION}`.

## [{VERSION}] - {DATE}

### Added

- Typed TOML configuration schema v2 with named profiles, portable user intent, deterministic schema-v1 migration, and `--profile` selection.
- Canonical `ptymark shell -- COMMAND` interactive execution while retaining the Alpha.3 `ptymark -- COMMAND` compatibility form.
- Structural SVG validation, bounded maintained XML parsing, typed managed-bundle inspection, and standard platform-directory and executable resolution.

### Changed

- Resolved executable paths, managed-bundle ownership, and installation provenance now live in machine-local install state instead of being copied into portable user configuration.
- Hard process, parser, terminal-control, artifact, and pending-output limits are centralized as internal policy and cannot be weakened through user TOML.
- Hand-written CLI parsing, doctor JSON serialization, XML tokenization, temporary-directory allocation, and LRU recency bookkeeping were replaced with maintained crates where the product contract could be preserved.
- CI now validates product-owned Rust, installer, renderer, Docker, and release surfaces directly; inherited repository-template and AgentCanon suites were removed from the product gate.

### Fixed

- Config and install-state publication is recoverable as one ownership-scoped transaction, preventing mixed new/old installation metadata after a failed commit.
- Unterminated or oversized terminal control sequences are bounded and remain on the raw byte-exact path.
- Renderer output containing an incidental `<svg` substring is no longer accepted as a valid SVG artifact.
- Cache recency updates are O(1), source-bearing keys are not duplicated for ordering, and disabled caches no longer report policy misses.

### Compatibility

- Schema-v1 TOML remains readable and can be normalized with `ptymark config migrate`; schema-v2 is the canonical emitted form.
- Existing `ptymark -- COMMAND`, source, safe, private, strict, PTY/ConPTY, exact-source fallback, and source-only distribution contracts remain supported.

### Known limitations

- This remains an alpha release. Guided first-run setup, complete CJK/grapheme/accessibility behavior, persistent renderer workers/cache, signed package channels, and terminal image protocols remain follow-up work.

"""
    write("CHANGELOG.md", text[:start] + section + text[next_release:])


def prepare_readme() -> None:
    text = read("README.md")
    text = text.replace("git clone --recurse-submodules ", "git clone ")
    text = text.replace(
        "- native Unix PTY and Windows ConPTY sessions through `ptymark -- COMMAND`;",
        "- native Unix PTY and Windows ConPTY sessions through canonical `ptymark shell -- COMMAND`, with `ptymark -- COMMAND` retained as an Alpha compatibility form;",
    )
    text = text.replace(
        "- installation-time path normalization and absolute-path configuration;",
        "- portable named-profile configuration separated from machine-local resolved installation state;",
    )
    text = text.replace(
        "- Docker plus Ubuntu, macOS, and Windows GitHub Actions validation.",
        "- product-owned Docker plus Ubuntu, macOS, and Windows GitHub Actions validation without inherited template suites.",
    )
    write("README.md", text)

    replace_section(
        "README.md",
        "## Interactive use",
        "## Pipe-oriented execution",
        """
The canonical interactive command is:

```text
ptymark [--config PATH] [--profile NAME] shell [OPTIONS] -- COMMAND [ARG...]
```

Examples:

```bash
ptymark shell -- bash
ptymark shell -- python
ptymark --profile plain shell -- cargo test
ptymark shell --source -- bash
ptymark shell --safe -- bash
ptymark shell --private -- bash
```

The Alpha.3 form remains a compatibility alias in Alpha.4:

```bash
ptymark -- bash
```

The command runs in a native Unix PTY or Windows ConPTY. Ptymark forwards input, propagates size changes, filters only safe child-output regions, and returns the child exit status. Child argv remains an `OsString` sequence after `--`; Ptymark does not build a shell command string.

Session modes change only pre-display policy:

- `--source` keeps explicit block detection but emits each complete block's exact source;
- `--safe` bypasses semantic detection, source-format conversion, engines, presentation, and cache;
- `--private` keeps rendering but disables the process-local artifact cache;
- `--allow-nested` permits deliberate development/debug nesting while accidental nesting remains rejected.

`--source` and `--safe` are mutually exclusive. See [`documents/interactive-session.md`](documents/interactive-session.md).
""",
    )

    replace_section(
        "README.md",
        "## Configuration",
        "## Renderer installation and isolation",
        """
Alpha.4 uses strict TOML schema v2. The user file records portable intent; resolved engine paths and ownership remain in machine-local installation state.

```toml
schema_version = 2
default_profile = "default"

[profiles.default.session]
mode = "render"             # render | source
strict = false

[profiles.default.detection]
mermaid = true
math = true                 # TeX and OpenMath

[profiles.default.presentation]
mode = "auto"              # auto | symbols | plain | source
color = "auto"             # auto | always | never
fallback_columns = 80

[profiles.default.cache]
backend = "memory"         # memory | none
max_entries = 128
max_bytes = 33554432

[profiles.default.engines.mermaid]
provider = "auto"          # auto | preview | source | managed | external

[profiles.default.engines.math]
provider = "auto"

[profiles.default.engines.presenter]
provider = "auto"          # auto | managed | external
```

Use `provider = "external"` together with an explicit `program` only when the executable choice itself is user intent. Installer-discovered or managed absolute paths are stored in `install.toml`, matched to the normalized user-config digest, and are not emitted by `ptymark config show`.

Named profiles provide the extension boundary for future presentation and engine policy without adding disconnected top-level flags:

```bash
ptymark --profile plain config check
ptymark --profile plain shell -- "$SHELL" -l
```

Schema-v1 files remain readable. Inspect or normalize them explicitly:

```bash
ptymark --config ~/.config/ptymark/config.toml config check
ptymark --config ~/.config/ptymark/config.toml config migrate
ptymark --config ~/.config/ptymark/config.toml config migrate --write
```

Hard renderer deadlines, artifact/output caps, OpenMath limits, terminal-control bounds, process polling, and PTY recovery timing are internal versioned policy. User TOML can choose stricter or lower-cost behavior but cannot weaken that safety floor.

`profiles.<name>.detection.math` governs dollar-sign block math, `math|latex|tex` fences, and `openmath` fences. OpenMath does not add another engine or installer role. See [`examples/README.md`](examples/README.md) and [`documents/alpha4-design.md`](documents/alpha4-design.md).
""",
    )

    text = read("README.md")
    text = text.replace(
        "  -> normalizes selected executables to native absolute paths\n  -> writes config.toml and install.toml atomically",
        "  -> records resolved executables in machine-local install state\n  -> commits portable config and ownership-scoped state transactionally",
    )
    text = text.replace(
        "Use explicit `--mermaid`, `--math`, and `--presenter` paths to replace one role while preserving unrelated settings.",
        "Use explicit `--mermaid`, `--math`, and `--presenter` values to change one profile role while preserving unrelated user settings. Resolved paths remain inspectable through `install status`, `engine check`, and `doctor`.",
    )
    write("README.md", text)


def prepare_examples() -> None:
    write(
        "examples/external-engines.toml",
        """# @dependency-start
# contract configuration
# responsibility Demonstrates explicit external renderer selection in schema v2.
# upstream design ../documents/ptymark-installer.md executable discovery contract
# downstream implementation ../src/config.rs configuration parser
# downstream implementation ../src/engine.rs selected engine roles
# @dependency-end

schema_version = 2
default_profile = "external"

[profiles.external.session]
mode = "render"
strict = false

[profiles.external.detection]
mermaid = true
math = true

[profiles.external.presentation]
mode = "symbols"
color = "auto"
fallback_columns = 100

[profiles.external.cache]
backend = "memory"
max_entries = 128
max_bytes = 33554432

# A bare name is resolved through PATH. An absolute program is also accepted.
[profiles.external.engines.mermaid]
provider = "external"
program = "mmdc"

# Keep math dependency-free for this profile.
[profiles.external.engines.math]
provider = "preview"

[profiles.external.engines.presenter]
provider = "external"
program = "chafa"
""",
    )

    text = read("examples/README.md")
    text = text.replace(
        "OpenMath shares `[detection].math` and `[engines.math]`; it does not require another configuration section or executable role.",
        "OpenMath shares `profiles.<name>.detection.math` and `profiles.<name>.engines.math`; it does not require another configuration section or executable role.",
    )
    text = text.replace(
        "GUI applications may not inherit the same environment as an interactive shell. Installer-generated absolute paths avoid renderer PATH ambiguity; explicit `PTYMARK_BINARY` and `PTYMARK_CONFIG` values remain the most predictable launcher setup.",
        "GUI applications may not inherit the same environment as an interactive shell. Managed and installer-discovered renderer paths are resolved from machine-local install state; explicit `PTYMARK_BINARY` and `PTYMARK_CONFIG` values remain the most predictable launcher setup.",
    )
    write("examples/README.md", text)


def prepare_installer_doc() -> None:
    text = read("documents/ptymark-installer.md")
    text = text.replace(
        "    -> atomically write runtime configuration and installation state",
        "    -> transactionally commit portable user configuration and machine-local installation state",
    )
    text = text.replace(
        "Linux, macOS, and WSL produce the same semantic configuration.",
        "Linux, macOS, and WSL produce the same portable user intent while retaining host-native resolved state.",
    )
    write("documents/ptymark-installer.md", text)

    replace_section(
        "documents/ptymark-installer.md",
        "## 4. Path contract",
        "## 5. One-command flow",
        """
Paths and preferences have explicit owners.

| Value | Owner | Stored form |
| --- | --- | --- |
| core binary | platform frontend | host-native absolute path |
| config path | platform frontend | host-native absolute path |
| state path | platform frontend | host-native absolute path |
| managed bundle root | platform frontend | host-native absolute path |
| user engine preference | user config | provider plus optional explicit program |
| discovered/managed engine path | install state | canonical host-native absolute path |
| temporary files and process metadata | runtime | never serialized as user preference |

Rules:

1. schema-v2 user configuration records portable product intent;
2. `external` may carry an absolute path or bare command name because that program is an explicit user choice;
3. `auto` and `managed` do not copy discovered paths into user TOML;
4. relative paths containing directory components are rejected;
5. Git Bash/MSYS/Cygwin paths are converted before the Rust resolver sees them;
6. WSL paths remain Linux paths;
7. install state is accepted only when its config path and normalized user-config digest match;
8. normal rendering performs no candidate ranking, installation, or network operation.
""",
    )

    replace_section(
        "documents/ptymark-installer.md",
        "## 5. One-command flow",
        "## 6. Engine slots and selection",
        """
On a new installation, the frontend performs:

```text
1. cargo install --locked --force --path REPOSITORY
2. resolve platform config/data/state destinations
3. inspect explicit options and visible system commands
4. determine whether any renderer role is missing
5. install or reuse the versioned managed bundle when allowed
6. build portable provider choices plus a machine-local resolved inventory
7. invoke `ptymark install resolve`
8. commit user config and install state as one recoverable transaction
9. invoke `ptymark install status`
```

An ordinary rerun preserves unrelated profiles and preferences. `--reprobe` or `-Reprobe` refreshes the machine-local resolution; `--reset` deliberately starts from the schema-v2 defaults. A failed managed installation or failed pair commit must not make a new configuration appear complete.
""",
    )


def prepare_release_notes() -> None:
    write(
        f"release-notes/{VERSION}.md",
        f"""# ptymark v{VERSION}

`v{VERSION}` is a source-only prerelease focused on typed configuration, machine-local installation ownership, bounded standard infrastructure, and a smaller product-specific CI surface.

## Highlights

- Introduces TOML configuration schema v2 with named profiles and deterministic schema-v1 migration.
- Separates portable `UserConfig` from resolved `InstallState`, per-session CLI overrides, and immutable internal safety policy.
- Makes `ptymark shell -- COMMAND` the canonical interactive command while retaining `ptymark -- COMMAND` as an Alpha compatibility form.
- Replaces hand-written CLI parsing, doctor JSON, generic XML tokenization, temporary directories, and LRU recency bookkeeping with maintained Rust crates.
- Adds bounded terminal-control parsing and structural SVG-root/namespace validation.
- Makes config/install-state publication recoverable and ownership-scoped.
- Removes inherited AgentCanon, Python/Jupyter template, and generic repository CI surfaces from the Ptymark product repository and gates only product-owned contracts.

## Configuration migration

Schema-v1 files remain readable. The canonical schema-v2 form stores user intent only:

```bash
ptymark --config ~/.config/ptymark/config.toml config check
ptymark --config ~/.config/ptymark/config.toml config migrate
ptymark --config ~/.config/ptymark/config.toml config migrate --write
```

Installer-discovered and managed executable paths are kept in machine-local installation state. `ptymark config show` therefore remains portable, while `ptymark install status`, `ptymark engine check`, and `ptymark doctor` report resolved local state.

## Install from the released source

```bash
git clone --branch v{VERSION} https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
bash scripts/installer.sh
```

Then verify:

```bash
ptymark --version
ptymark install status
ptymark config check
ptymark engine check
ptymark doctor
```

On Windows, use the documented PowerShell, cmd.exe, or Windows-Bash source-install route.

## Source-only distribution

This release intentionally contains no project-uploaded executable archives, installer bundles, renderer bundles, binary checksums, binary manifests, attestations, or executable-bearing artifacts. GitHub provides the immutable tag, these notes, and its generated source snapshots only.

Local builds and third-party packages are not automatically trusted or endorsed. Review the tag, lockfiles, toolchain, dependencies, and downstream packaging channel appropriate to the environment.

## Compatibility and safety

- PTY/ConPTY byte transport, exact-source fallback, source/safe/private modes, active-session marking, nesting rejection, and child exit-status behavior remain compatible.
- Hard deadlines, parser limits, artifact caps, pending-output bounds, and cleanup timing cannot be weakened through user TOML.
- Invalid, incomplete, timed-out, cancelled, or presentation-failed artifacts remain ineligible for cache insertion.

## Known limitations

- This remains an alpha release.
- Guided setup, complete CJK/grapheme/accessibility behavior, persistent renderer workers/cache, signed package channels, and terminal image protocols remain follow-up work.
""",
    )


def main() -> None:
    prepare_cargo()
    prepare_changelog()
    prepare_readme()
    prepare_examples()
    prepare_installer_doc()
    prepare_release_notes()
    Path(__file__).unlink()


if __name__ == "__main__":
    main()
