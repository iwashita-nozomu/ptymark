#!/usr/bin/env python3
"""Publish reviewed ptymark roadmap pages to the GitHub Wiki once.

This script lives only on a temporary diagnostic branch. It reads canonical
GitHub issues and release metadata, updates the Wiki Git repository, verifies
the pushed pages, and writes non-sensitive evidence for the workflow artifact.
"""

from __future__ import annotations

import datetime as dt
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
from typing import Any

REPOSITORY = os.environ["GITHUB_REPOSITORY"]
RELEASE_TAG = "v0.1.0-alpha.1"
WORK_ROOT = Path("wiki-publication")
EVIDENCE = WORK_ROOT / "evidence"
WIKI = WORK_ROOT / "wiki"
VERIFY = WORK_ROOT / "verify"
ASKPASS = WORK_ROOT / "askpass.sh"
WIKI_URL = f"https://github.com/{REPOSITORY}.wiki.git"


def run(
    argv: list[str],
    *,
    cwd: Path | None = None,
    capture: bool = False,
    check: bool = True,
    git_auth: bool = False,
) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    if git_auth:
        env.update(
            {
                "GIT_ASKPASS": str(ASKPASS.resolve()),
                "GIT_TERMINAL_PROMPT": "0",
            }
        )
    result = subprocess.run(
        argv,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
        check=False,
    )
    if check and result.returncode != 0:
        if capture:
            print(result.stdout, end="", file=sys.stdout)
            print(result.stderr, end="", file=sys.stderr)
        raise RuntimeError(f"command failed with exit code {result.returncode}: {argv[0]}")
    return result


def gh_json(endpoint: str) -> dict[str, Any]:
    result = run(["gh", "api", endpoint], capture=True)
    return json.loads(result.stdout)


def write(path: Path, content: str) -> None:
    path.write_text(content.rstrip() + "\n", encoding="utf-8")


def main() -> None:
    shutil.rmtree(WORK_ROOT, ignore_errors=True)
    EVIDENCE.mkdir(parents=True)

    write(
        ASKPASS,
        """#!/usr/bin/env bash
case "$1" in
  *Username*) printf '%s\\n' 'x-access-token' ;;
  *Password*) printf '%s\\n' "$GH_TOKEN" ;;
  *) printf '\\n' ;;
esac
""",
    )
    ASKPASS.chmod(ASKPASS.stat().st_mode | stat.S_IXUSR)

    refs_before = run(["git", "ls-remote", WIKI_URL], capture=True, git_auth=True)
    write(EVIDENCE / "refs-before.txt", refs_before.stdout)
    run(["git", "clone", WIKI_URL, str(WIKI)], git_auth=True)

    existing = sorted(path.name for path in WIKI.iterdir() if path.is_file())
    write(EVIDENCE / "existing-pages.txt", "\n".join(existing))

    roadmap = gh_json(f"repos/{REPOSITORY}/issues/34")
    adoptability = gh_json(f"repos/{REPOSITORY}/issues/48")
    release = gh_json(f"repos/{REPOSITORY}/releases/tags/{RELEASE_TAG}")
    main_ref = gh_json(f"repos/{REPOSITORY}/git/ref/heads/main")
    main_sha = main_ref["object"]["sha"]
    generated = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()

    home = f"""# ptymark Wiki

ptymark is a pre-display renderer for explicit semantic blocks in terminal output. It preserves ordinary terminal traffic and unsafe control-oriented regions byte-for-byte, while supported Mermaid and block-math input can be rendered before display.

## Start here

- [[Roadmap]] — dependency-ordered release trains and exit gates.
- [[Adoptability]] — user-journey acceptance ledger.
- [[Release Status|Release-Status]] — current published release, source identity, and assets.
- [Repository](https://github.com/{REPOSITORY})
- [Issues](https://github.com/{REPOSITORY}/issues)
- [Releases](https://github.com/{REPOSITORY}/releases)

## Current baseline

The first immutable prerelease, `{RELEASE_TAG}`, is published from reviewed `main`. The released surface includes file/stream preview, pipe-oriented command filtering, native Unix PTY and Windows ConPTY sessions, per-session `--source`, `--safe`, and `--private` modes, package-local installers, checksums, a release manifest, and build provenance.

## Source-of-truth policy

- GitHub Issue [#34](https://github.com/{REPOSITORY}/issues/34) is the canonical execution roadmap.
- GitHub Issue [#48](https://github.com/{REPOSITORY}/issues/48) is the canonical adoptability ledger.
- Wiki pages are readable mirrors and navigation pages. Approve implementation and acceptance changes in their canonical issues before refreshing the Wiki.

_Last refreshed: {generated}_
"""

    roadmap_page = f"""# {roadmap['title']}

> Canonical source: GitHub Issue [#{roadmap['number']}]({roadmap['html_url']}).  
> This Wiki page is a readable mirror; update the issue first when scope, ordering, or exit gates change.  
> Refreshed: {generated}

{roadmap.get('body') or ''}
"""

    adoptability_page = f"""# {adoptability['title']}

> Canonical source: GitHub Issue [#{adoptability['number']}]({adoptability['html_url']}).  
> Check items only from merged or released evidence.  
> Refreshed: {generated}

{adoptability.get('body') or ''}
"""

    asset_rows = "\n".join(
        f"| `{asset['name']}` | {asset.get('size', 0):,} | "
        f"`{asset.get('digest') or 'n/a'}` |"
        for asset in release.get("assets", [])
    )
    release_status = f"""# Release Status

## Current release

| Field | Value |
| --- | --- |
| Tag | `{release['tag_name']}` |
| Name | {release.get('name') or release['tag_name']} |
| Published | `{release.get('published_at') or 'n/a'}` |
| Prerelease | `{str(bool(release.get('prerelease'))).lower()}` |
| Draft | `{str(bool(release.get('draft'))).lower()}` |
| Current `main` | `{main_sha}` |
| Release target | `{release.get('target_commitish') or 'n/a'}` |

The release workflow rebuilds Linux, macOS, and Windows archives from the immutable release source, smoke-tests them, generates adjacent and aggregate SHA-256 records, creates a machine-readable manifest, verifies build provenance, and then publishes the prerelease.

## Published assets

| Asset | Size (bytes) | GitHub digest |
| --- | ---: | --- |
{asset_rows}

## Verification contract

1. Verify the archive using `SHA256SUMS` or its adjacent `.sha256` file.
2. Confirm `release-manifest.json` names the expected tag, version, source commit, platform, architecture, size, and digest.
3. Verify GitHub build provenance for the downloaded archive.
4. Never replace bytes under an existing tag; publish a higher version for corrections.

See the repository's `documents/release.md` for recovery and rollback rules.

_Last refreshed: {generated}_
"""

    sidebar = """**ptymark Wiki**

- [[Home]]
- [[Roadmap]]
- [[Adoptability]]
- [[Release Status|Release-Status]]

**Project**

- [Repository](https://github.com/iwashita-nozomu/ptymark)
- [Issues](https://github.com/iwashita-nozomu/ptymark/issues)
- [Releases](https://github.com/iwashita-nozomu/ptymark/releases)
"""

    pages = {
        "Home.md": home,
        "Roadmap.md": roadmap_page,
        "Adoptability.md": adoptability_page,
        "Release-Status.md": release_status,
        "_Sidebar.md": sidebar,
    }
    for name, content in pages.items():
        write(WIKI / name, content)

    summary = {
        "generated_at": generated,
        "repository": REPOSITORY,
        "main_sha": main_sha,
        "release_tag": release["tag_name"],
        "roadmap_issue": roadmap["number"],
        "adoptability_issue": adoptability["number"],
        "pages": sorted(pages),
    }
    write(EVIDENCE / "publication-summary.json", json.dumps(summary, indent=2))
    write(EVIDENCE / "generated-pages.txt", "\n".join(sorted(pages)))

    run(["git", "config", "user.name", "github-actions[bot]"], cwd=WIKI)
    run(
        [
            "git",
            "config",
            "user.email",
            "41898282+github-actions[bot]@users.noreply.github.com",
        ],
        cwd=WIKI,
    )
    run(["git", "add", *sorted(pages)], cwd=WIKI)
    run(["git", "diff", "--cached", "--check"], cwd=WIKI)
    diff_stat = run(["git", "diff", "--cached", "--stat"], cwd=WIKI, capture=True)
    write(EVIDENCE / "diff-stat.txt", diff_stat.stdout)

    changed = run(["git", "diff", "--cached", "--quiet"], cwd=WIKI, check=False).returncode != 0
    if changed:
        run(["git", "commit", "-m", "docs: publish reviewed project roadmap"], cwd=WIKI)
        run(["git", "push", "origin", "HEAD:master"], cwd=WIKI, git_auth=True)
    wiki_head = run(["git", "rev-parse", "HEAD"], cwd=WIKI, capture=True).stdout.strip()
    write(
        EVIDENCE / "publish-result.txt",
        f"changed={str(changed).lower()}\ncommit={wiki_head}",
    )
    commit_info = run(["git", "log", "-1", "--format=fuller"], cwd=WIKI, capture=True)
    write(EVIDENCE / "wiki-commit.txt", commit_info.stdout)

    run(["git", "clone", WIKI_URL, str(VERIFY)], git_auth=True)
    for name in pages:
        path = VERIFY / name
        if not path.is_file() or path.stat().st_size == 0:
            raise RuntimeError(f"published Wiki page is missing or empty: {name}")
    if "Release train 1" not in (VERIFY / "Roadmap.md").read_text(encoding="utf-8"):
        raise RuntimeError("Roadmap page does not contain the active release train")
    if RELEASE_TAG not in (VERIFY / "Release-Status.md").read_text(encoding="utf-8"):
        raise RuntimeError("Release Status page does not contain the release tag")
    verified_head = run(["git", "rev-parse", "HEAD"], cwd=VERIFY, capture=True).stdout.strip()
    write(EVIDENCE / "verified-head.txt", verified_head)
    verified_pages = sorted(path.name for path in VERIFY.glob("*.md"))
    write(EVIDENCE / "verified-pages.txt", "\n".join(verified_pages))

    ASKPASS.unlink(missing_ok=True)
    print(json.dumps(summary, indent=2))
    print(f"wiki_head={verified_head}")


if __name__ == "__main__":
    main()
