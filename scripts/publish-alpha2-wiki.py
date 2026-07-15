#!/usr/bin/env python3

from __future__ import annotations

import base64
import json
import os
import shutil
import subprocess
import textwrap
from pathlib import Path

REPOSITORY = os.environ["GITHUB_REPOSITORY"]
TOKEN = os.environ["GH_TOKEN"]
ROOT = Path.cwd()
WIKI = ROOT / "wiki-worktree"
VERIFY = ROOT / "wiki-fresh-clone"
EVIDENCE = ROOT / "wiki-evidence"
WIKI_URL = f"https://github.com/{REPOSITORY}.wiki.git"


def auth_env() -> dict[str, str]:
    env = os.environ.copy()
    credential = base64.b64encode(f"x-access-token:{TOKEN}".encode()).decode()
    env.update(
        {
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "http.https://github.com/.extraheader",
            "GIT_CONFIG_VALUE_0": f"AUTHORIZATION: basic {credential}",
        }
    )
    return env


def run(*args: str, cwd: Path | None = None, auth: bool = False) -> str:
    result = subprocess.run(
        args,
        cwd=cwd,
        env=auth_env() if auth else os.environ.copy(),
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(args)}\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )
    return result.stdout.strip()


def page(text: str) -> str:
    return textwrap.dedent(text).strip() + "\n"


PAGES = {
    "Home.md": page(
        """
        # ptymark Wiki

        [English](Home) · [日本語](Home-ja)

        Ptymark is a terminal-aware pre-display renderer for explicitly fenced Mermaid and math blocks. The native Unix PTY / Windows ConPTY runtime preserves ordinary terminal traffic, control sequences, and alternate-screen applications while semantic work is bounded behind an exact-source fallback.

        ## Current release

        **`v0.1.0-alpha.2` is published and independently verified.**

        It adds:

        - `ptymark doctor` and the versioned `ptymark.doctor.v1` report;
        - public-safe redaction and atomic support reports;
        - a ten-second external render/presentation deadline;
        - a one-MiB pending-output bound;
        - ordered exact-source recovery and no cache admission after failure;
        - Linux, macOS, and Windows archives with checksums, manifest, and provenance.

        ```mermaid
        flowchart LR
            A1[v0.1.0-alpha.1\nPTY/ConPTY runtime] --> A2[v0.1.0-alpha.2\ndoctor and bounded recovery]
            A2 --> A3[v0.1.0-alpha.3\nguided setup and accessible text]
            A3 --> B1[v0.1.0-beta.1\nlifecycle readiness]
        ```

        ## Next release

        `v0.1.0-alpha.3` is tracked in [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90). It focuses on guided first-run verification, collision-safe WezTerm onboarding, and dependable text/plain/source behavior for CJK, emoji, narrow terminals, no-color, SSH/tmux, logs, and screen readers.

        ## Start here

        - [Roadmap](Roadmap)
        - [Next Release](Next-Release)
        - [Adoptability](Adoptability)
        - [Release Status](Release-Status)
        - [Troubleshooting](Troubleshooting)

        Canonical roadmap decisions live in [Issue #34](https://github.com/iwashita-nozomu/ptymark/issues/34); the user-journey ledger lives in [Issue #48](https://github.com/iwashita-nozomu/ptymark/issues/48).
        """
    ),
    "Home-ja.md": page(
        """
        # ptymark Wiki

        [English](Home) · [日本語](Home-ja)

        Ptymark は、明示的にフェンスされた Mermaid・数式ブロックを、表示直前に安全に処理するターミナル向けレンダラーです。Unix PTY / Windows ConPTY の通常出力、制御シーケンス、alternate screen はそのまま維持し、意味ブロックの処理は必ず exact source fallback の境界内で行います。

        ## 現在のリリース

        **`v0.1.0-alpha.2` は公開済みで、公開物の独立検証も完了しています。**

        主な追加点:

        - `ptymark doctor` と versioned schema `ptymark.doctor.v1`;
        - 公開向けredactionとatomicなsupport report;
        - 外部render/presentationの10秒hard deadline;
        - pending outputの1 MiB上限;
        - 順序を保ったexact-source復旧と、失敗結果のcache非登録;
        - checksum・manifest・provenance付きLinux/macOS/Windows archive。

        ```mermaid
        flowchart LR
            A1[v0.1.0-alpha.1\nPTY/ConPTY runtime] --> A2[v0.1.0-alpha.2\ndoctorとbounded recovery]
            A2 --> A3[v0.1.0-alpha.3\nguided setupとaccessible text]
            A3 --> B1[v0.1.0-beta.1\nlifecycle readiness]
        ```

        ## 次のリリース

        `v0.1.0-alpha.3` は [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90) で追跡します。初回利用を検証するguided setup、既存設定を壊さないWezTerm onboarding、CJK・emoji・狭幅・no-color・SSH/tmux・log・screen reader向けのtext/plain/source表示が中心です。

        ## 入口

        - [ロードマップ](Roadmap-ja)
        - [次のリリース](Next-Release-ja)
        - [利用定着度](Adoptability-ja)
        - [リリース状況](Release-Status-ja)
        - [トラブルシューティング](Troubleshooting-ja)

        ロードマップの正本は [Issue #34](https://github.com/iwashita-nozomu/ptymark/issues/34)、利用者journeyの受入台帳は [Issue #48](https://github.com/iwashita-nozomu/ptymark/issues/48) です。
        """
    ),
    "Roadmap.md": page(
        """
        # Roadmap

        [English](Roadmap) · [日本語](Roadmap-ja)

        The canonical roadmap is [Issue #34](https://github.com/iwashita-nozomu/ptymark/issues/34). This page is its readable mirror.

        ## Released baseline — v0.1.0-alpha.2

        Source commit: `c4f23c924c855094d09077c293d06b2a6157250d`

        Released capabilities include doctor v1, redacted support reports, stable finding/status semantics, a ten-second external-attempt deadline, one-MiB pending-output bound, ordered source recovery, no failed-result cache admission, three-platform packages, checksums, manifest, and provenance.

        ## Active train — v0.1.0-alpha.3

        Tracker: [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90)

        Theme: **guided setup, WezTerm onboarding, and accessible text presentation**.

        ```mermaid
        flowchart LR
            D[#16 compatibility inventory] --> W[#14 WezTerm generation]
            D --> S[#44 guided setup]
            P[#10 and #46 text/accessibility] --> S
            W --> S
            S --> E[#18 canonical examples]
            P --> E
            E --> RC[#90 release candidate]
            RC --> R[v0.1.0-alpha.3]
            R --> V[downloaded verification]
        ```

        ### Execution order

        1. #16: typed dependency/runtime/browser compatibility needed by setup and doctor.
        2. #10/#46: auto/symbols/text/plain/source behavior, CJK/grapheme width, no-color, screen-reader and remote/log fallbacks.
        3. #14: resolved and collision-safe WezTerm command/Lua generation.
        4. #44: `ptymark setup`, `--check-only`, `--wezterm`, and `--print-wezterm-lua`.
        5. #18: canonical machine-tested examples and documentation drift checks.
        6. #90: versioning, packages, immutable publication, and independent downloaded-asset verification.

        ### Alpha.3 exit gate

        - a fresh Linux/macOS/Windows package reaches a verified Mermaid/math and PTY/ConPTY result without manual path editing;
        - check-only performs no write/install/download/network action;
        - existing WezTerm configuration is not overwritten;
        - Japanese/CJK/emoji/grapheme/no-color/SSH/tmux/log/screen-reader fixtures preserve following output;
        - exact recent source is retrievable without automatic persistent source storage;
        - all alpha.2 doctor, redaction, timeout, ordering, PTY/ConPTY, and package guarantees remain green;
        - published archives, checksums, manifest, and attestations verify independently.

        ## Later trains

        ```mermaid
        flowchart TD
            A3[v0.1.0-alpha.3\nadoptable alpha] --> B1[v0.1.0-beta.1\nupgrade rollback uninstall purge]
            B1 --> C1[stable configuration v1]
            C1 --> P[production workers cache and performance]
            P --> I[capability-safe terminal images]
            B1 --> S[signed native distribution]
        ```

        Image protocols, persistent workers, lifecycle mutation, stable configuration v1, and signed package-manager channels are explicit non-goals for alpha.3.
        """
    ),
    "Roadmap-ja.md": page(
        """
        # ロードマップ

        [English](Roadmap) · [日本語](Roadmap-ja)

        正本は [Issue #34](https://github.com/iwashita-nozomu/ptymark/issues/34) です。このページは読みやすいmirrorです。

        ## 公開済みbaseline — v0.1.0-alpha.2

        Source commit: `c4f23c924c855094d09077c293d06b2a6157250d`

        doctor v1、redacted support report、stable finding/status、外部attemptの10秒deadline、pending outputの1 MiB上限、順序を保つsource復旧、失敗結果のcache非登録、3 OS package、checksum、manifest、provenanceが公開済みです。

        ## 現在のtrain — v0.1.0-alpha.3

        Tracker: [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90)

        Theme: **guided setup、WezTerm onboarding、accessible text presentation**。

        ```mermaid
        flowchart LR
            D[#16 compatibility inventory] --> W[#14 WezTerm generation]
            D --> S[#44 guided setup]
            P[#10 / #46 textとaccessibility] --> S
            W --> S
            S --> E[#18 canonical examples]
            P --> E
            E --> RC[#90 release candidate]
            RC --> R[v0.1.0-alpha.3]
            R --> V[downloaded verification]
        ```

        ### 実行順

        1. #16: setupとdoctorが共有するdependency/runtime/browser compatibility。
        2. #10/#46: auto/symbols/text/plain/source、CJK/grapheme幅、no-color、screen-reader、remote/log fallback。
        3. #14: resolved pathを使うcollision-safeなWezTerm command/Lua生成。
        4. #44: `ptymark setup`、`--check-only`、`--wezterm`、`--print-wezterm-lua`。
        5. #18: machine-testedなcanonical examplesとdocument drift防止。
        6. #90: versioning、package、immutable publication、公開assetの独立検証。

        ### Alpha.3 exit gate

        - freshなLinux/macOS/Windows packageから、手動path編集なしでMermaid/mathとPTY/ConPTYを検証できる;
        - check-onlyはwrite/install/download/networkを行わない;
        - 既存WezTerm設定を上書きしない;
        - Japanese/CJK/emoji/grapheme/no-color/SSH/tmux/log/screen-reader fixtureで後続出力を壊さない;
        - 自動persistent保存なしで直近sourceを取得できる;
        - alpha.2のdoctor/redaction/timeout/order/PTY/ConPTY/package保証が維持される;
        - 公開archive/checksum/manifest/attestationを独立検証できる。

        ## 後続train

        ```mermaid
        flowchart TD
            A3[v0.1.0-alpha.3\nadoptable alpha] --> B1[v0.1.0-beta.1\nupgrade rollback uninstall purge]
            B1 --> C1[stable configuration v1]
            C1 --> P[production workers cache performance]
            P --> I[capability-safe terminal images]
            B1 --> S[signed native distribution]
        ```

        image protocol、persistent worker、lifecycle mutation、stable configuration v1、signed package-manager channelはalpha.3の非目標です。
        """
    ),
    "Next-Release.md": page(
        """
        # Next Release — v0.1.0-alpha.3

        [English](Next-Release) · [日本語](Next-Release-ja)

        Canonical tracker: [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90)

        ## Product promise

        A fresh package can run one guided flow that identifies the binary/configuration, checks dependency compatibility, performs bounded real Mermaid/math and PTY/ConPTY tests, prints a safe launch command, and returns to doctor v1 for later diagnosis.

        Users also gain dependable symbols/text/plain/source behavior for CJK, emoji, combining characters, narrow terminals, no-color, SSH/tmux, logs, and screen-reader-oriented workflows.

        ## Public setup surface

        ```text
        ptymark setup
        ptymark setup --check-only
        ptymark setup --wezterm
        ptymark setup --print-wezterm-lua
        ```

        ```mermaid
        flowchart TD
            I[Fresh package] --> C[Validate config and install state]
            C --> D[Inspect renderer/runtime/browser compatibility]
            D --> M[Bounded Mermaid and math self-test]
            M --> P[Real PTY/ConPTY child smoke]
            P --> W[Generate or print WezTerm integration]
            W --> O[Show launch command and doctor follow-up]
            C -->|failure| R[Exact stage and remedy]
            D -->|failure| R
            M -->|failure| R
            P -->|failure| R
        ```

        ## Safety rules

        - check-only is read-only and network-free;
        - no shell profile, global PATH, terminal config, or unrelated file is changed automatically;
        - optional writes are explicit, atomic, destination-scoped, and idempotent;
        - existing WezTerm keys/menu entries are preserved and collisions are reported;
        - no shell command strings or `eval` paths;
        - setup reuses alpha.2 deadlines, findings, redaction, and doctor status.

        ## Explicit non-goals

        - terminal image protocols;
        - persistent workers/concurrency expansion;
        - stable configuration v1;
        - upgrade/rollback/uninstall/purge;
        - signed package-manager channels;
        - automatic issue upload or source/secret collection.

        ## Completion

        Alpha.3 is complete only after an immutable three-platform prerelease is published and a fresh downloaded package passes the complete setup, WezTerm, text/CJK/accessibility, doctor, safe/private, checksum, manifest, and provenance journey.
        """
    ),
    "Next-Release-ja.md": page(
        """
        # 次のリリース — v0.1.0-alpha.3

        [English](Next-Release) · [日本語](Next-Release-ja)

        正本tracker: [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90)

        ## Product promise

        fresh packageから1つのguided flowを実行し、binary/config、dependency compatibility、boundedなMermaid/mathとPTY/ConPTYの実テスト、safeなlaunch command、後続診断用doctorまで到達できるようにします。

        CJK、emoji、combining character、狭いterminal、no-color、SSH/tmux、log、screen-reader向けに、symbols/text/plain/source表示を予測可能にします。

        ## 公開setup surface

        ```text
        ptymark setup
        ptymark setup --check-only
        ptymark setup --wezterm
        ptymark setup --print-wezterm-lua
        ```

        ```mermaid
        flowchart TD
            I[Fresh package] --> C[configとinstall stateを検証]
            C --> D[renderer/runtime/browser compatibility]
            D --> M[bounded Mermaid/math self-test]
            M --> P[実PTY/ConPTY child smoke]
            P --> W[WezTerm integrationを生成/表示]
            W --> O[launch commandとdoctorを表示]
            C -->|failure| R[失敗stageとremedy]
            D -->|failure| R
            M -->|failure| R
            P -->|failure| R
        ```

        ## Safety rules

        - check-onlyはread-onlyかつnetwork-free;
        - shell profile、global PATH、terminal config、無関係なfileを自動変更しない;
        - optional writeは明示的・atomic・destination-scoped・idempotent;
        - 既存WezTerm key/menuを保存し、collisionを報告する;
        - shell command stringや`eval`を使わない;
        - alpha.2のdeadline、finding、redaction、doctor statusを再利用する。

        ## 明示的な非目標

        - terminal image protocol;
        - persistent worker/concurrency拡張;
        - stable configuration v1;
        - upgrade/rollback/uninstall/purge;
        - signed package-manager channel;
        - 自動issue uploadやsource/secret収集。

        ## 完了条件

        immutableな3 OS prereleaseを公開し、downloadしたfresh packageでsetup、WezTerm、text/CJK/accessibility、doctor、safe/private、checksum、manifest、provenanceの全journeyを独立検証して初めて完了です。
        """
    ),
    "Adoptability.md": page(
        """
        # Adoptability

        [English](Adoptability) · [日本語](Adoptability-ja)

        Canonical ledger: [Issue #48](https://github.com/iwashita-nozomu/ptymark/issues/48)

        ## Completed through v0.1.0-alpha.2

        - install from immutable Linux/macOS/Windows archives without Rust or global Node;
        - verify checksums, manifest, and build provenance;
        - launch real Unix PTY / Windows ConPTY sessions;
        - use source, safe, and private one-off modes;
        - diagnose ready/degraded/unusable state with doctor v1;
        - share a deterministic redacted report;
        - distinguish missing/incompatible components and bounded renderer failures;
        - recover exact source in order after timeout/output/process/presentation failure.

        ```mermaid
        flowchart LR
            D[Discover] --> I[Install]
            I --> U[Use PTY/ConPTY]
            U --> X[Diagnose with doctor]
            X --> R[Recover exact source]
            R --> S[Share redacted report]
            S --> G[Guided setup and accessible text — alpha.3]
            G --> L[Lifecycle — beta.1]
        ```

        ## Remaining for v0.1.0-alpha.3

        - one guided first-run flow and real render;
        - generated/tested WezTerm onboarding;
        - deterministic symbols/text/plain/source modes;
        - CJK/emoji/grapheme/no-color/narrow-terminal correctness;
        - SSH/tmux/log/screen-reader paths;
        - ephemeral exact-source retrieval after visual presentation.

        ## Remaining beyond alpha.3

        - upgrade, rollback, uninstall, and purge;
        - stable configuration v1 and project trust/migration;
        - persistent workers and performance gates;
        - safe terminal image placement;
        - signed trusted distribution channels.
        """
    ),
    "Adoptability-ja.md": page(
        """
        # 利用定着度

        [English](Adoptability) · [日本語](Adoptability-ja)

        正本台帳: [Issue #48](https://github.com/iwashita-nozomu/ptymark/issues/48)

        ## v0.1.0-alpha.2までに完了

        - Rust/global NodeなしでimmutableなLinux/macOS/Windows archiveからinstall;
        - checksum、manifest、build provenanceの検証;
        - real Unix PTY / Windows ConPTY session;
        - source/safe/private one-off mode;
        - doctor v1によるready/degraded/unusable診断;
        - deterministicなredacted report共有;
        - missing/incompatible componentとbounded renderer failureの区別;
        - timeout/output/process/presentation failure後に順序を保つexact-source復旧。

        ```mermaid
        flowchart LR
            D[知る] --> I[install]
            I --> U[PTY/ConPTYを使う]
            U --> X[doctorで診断]
            X --> R[exact sourceへ復旧]
            R --> S[redacted reportを共有]
            S --> G[guided setupとaccessible text — alpha.3]
            G --> L[lifecycle — beta.1]
        ```

        ## v0.1.0-alpha.3で残るもの

        - 1つのguided first-run flowとreal render;
        - generated/testedなWezTerm onboarding;
        - deterministicなsymbols/text/plain/source mode;
        - CJK/emoji/grapheme/no-color/narrow-terminal correctness;
        - SSH/tmux/log/screen-reader path;
        - visual presentation後のephemeral exact-source retrieval。

        ## alpha.3以後

        - upgrade/rollback/uninstall/purge;
        - stable configuration v1とproject trust/migration;
        - persistent workerとperformance gate;
        - safeなterminal image placement;
        - signed trusted distribution channel。
        """
    ),
    "Release-Status.md": page(
        """
        # Release Status

        [English](Release-Status) · [日本語](Release-Status-ja)

        ## Current release: v0.1.0-alpha.2

        - Reviewed source commit: `c4f23c924c855094d09077c293d06b2a6157250d`
        - GitHub prerelease: published
        - Release workflow: completed successfully
        - Platforms: Linux x86_64, macOS aarch64, Windows x86_64
        - Assets: three archives, three adjacent `.sha256` files, `SHA256SUMS`, `release-manifest.json`
        - Build provenance: verified for all three archives

        ```mermaid
        flowchart LR
            M[Reviewed main] --> T[Annotated immutable tag]
            T --> L[Linux build and smoke]
            T --> A[macOS build and smoke]
            T --> W[Windows build and smoke]
            L --> C[Checksums and manifest]
            A --> C
            W --> C
            C --> P[Build provenance]
            P --> R[Immutable prerelease]
            R --> V[Independent downloaded verification]
        ```

        ## Archive SHA-256

        | Platform | SHA-256 |
        | --- | --- |
        | Linux x86_64 | `e0d8aba8a84161cdd652cb2227d55d6262eb6c754edb70d3f387fb50e813971b` |
        | macOS aarch64 | `74baad0647c51257834382f126149a76e0052fb4cff331886196ef7ea37c0354` |
        | Windows x86_64 | `2b1c561b2eb934fb5ef0861737ddf8dcb9c6d01eb934e24263d970d23f8b07d8` |

        ## Independent verification

        The published assets were downloaded after release and checked for:

        - aggregate and adjacent checksum agreement;
        - manifest version, tag, source commit, platform, architecture, size, and digest;
        - required package contents on all three platforms;
        - Linux `ptymark 0.1.0-alpha.2` version output;
        - Linux `ptymark.doctor.v1` output;
        - byte-exact `preview --source` and `preview --safe`;
        - provenance attestations for each archive.

        ## Next

        `v0.1.0-alpha.3` is tracked by [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90).
        """
    ),
    "Release-Status-ja.md": page(
        """
        # リリース状況

        [English](Release-Status) · [日本語](Release-Status-ja)

        ## 現在のリリース: v0.1.0-alpha.2

        - Reviewed source commit: `c4f23c924c855094d09077c293d06b2a6157250d`
        - GitHub prerelease: 公開済み
        - Release workflow: 成功
        - Platform: Linux x86_64、macOS aarch64、Windows x86_64
        - Asset: 3 archive、3 adjacent `.sha256`、`SHA256SUMS`、`release-manifest.json`
        - Build provenance: 3 archiveすべて検証済み

        ```mermaid
        flowchart LR
            M[Reviewed main] --> T[Annotated immutable tag]
            T --> L[Linux build/smoke]
            T --> A[macOS build/smoke]
            T --> W[Windows build/smoke]
            L --> C[checksum/manifest]
            A --> C
            W --> C
            C --> P[build provenance]
            P --> R[immutable prerelease]
            R --> V[downloaded assetの独立検証]
        ```

        ## Archive SHA-256

        | Platform | SHA-256 |
        | --- | --- |
        | Linux x86_64 | `e0d8aba8a84161cdd652cb2227d55d6262eb6c754edb70d3f387fb50e813971b` |
        | macOS aarch64 | `74baad0647c51257834382f126149a76e0052fb4cff331886196ef7ea37c0354` |
        | Windows x86_64 | `2b1c561b2eb934fb5ef0861737ddf8dcb9c6d01eb934e24263d970d23f8b07d8` |

        ## 独立検証

        公開後にassetを再downloadし、次を検証しました。

        - aggregate/adjacent checksum一致;
        - manifestのversion、tag、source commit、platform、architecture、size、digest;
        - 3 platformの必須package内容;
        - Linuxの`ptymark 0.1.0-alpha.2`;
        - Linuxの`ptymark.doctor.v1`;
        - byte-exactな`preview --source`と`preview --safe`;
        - 各archiveのprovenance attestation。

        ## 次

        `v0.1.0-alpha.3` は [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90) で追跡します。
        """
    ),
    "Troubleshooting.md": page(
        """
        # Troubleshooting

        [English](Troubleshooting) · [日本語](Troubleshooting-ja)

        ## Start with doctor

        ```text
        ptymark doctor
        ptymark doctor --json
        ptymark doctor --support-report ptymark-support.json
        ptymark doctor --config /path/to/ptymark.toml
        ```

        Doctor v1 is side-effect-free by default: it does not install, download, access the network, launch a renderer/browser/PTY child, or mutate configuration/state.

        | Status | Exit | Meaning |
        | --- | ---: | --- |
        | ready | 0 | selected configuration is usable |
        | degraded | 10 | usable through a documented fallback or without an optional capability |
        | unusable | 20 | selected strict configuration or required host cannot operate |

        ```mermaid
        flowchart TD
            S[Rendering or terminal problem] --> D[Run ptymark doctor]
            D --> R{Status}
            R -->|ready| P[Check the specific block or presenter]
            R -->|degraded| F[Use source/safe fallback and apply remedy]
            R -->|unusable| C[Fix config, install state, dependency, or host]
            P --> J[Attach redacted ptymark.doctor.v1 report]
            F --> J
            C --> J
        ```

        ## Immediate recovery modes

        ```text
        ptymark --source -- COMMAND
        ptymark --safe -- COMMAND
        ptymark --private -- COMMAND
        ptymark preview --source FILE
        ptymark preview --safe FILE
        ```

        - `--source`: detect supported fenced blocks but show exact source; no external renderer/presenter.
        - `--safe`: bypass semantic detection completely; no renderer, presenter, or cache.
        - `--private`: preserve rendering policy but disable automatic persistent source-bearing state.

        ## Renderer stalls and failures

        Alpha.2 bounds each external attempt to ten seconds and later pending output to one MiB. Non-strict failures commit exact source before releasing later output. Failed, timed-out, cancelled, invalid, or presentation-failed attempts do not enter the cache.

        Useful finding codes include:

        ```text
        render.timeout
        render.output_limit
        render.process_exit
        presentation.fallback
        engine.missing
        engine.incompatible
        browser.unavailable
        presenter.unsupported
        ```

        ## Redaction guarantee

        Default doctor/support output excludes semantic source, child environment, credentials/tokens/cookies, command history, raw source-bearing renderer stderr, identifying path prefixes, terminal-control bytes, invalid byte sequences, and unbounded diagnostic content.

        Review the report before posting it, but do not replace it with a full environment dump or raw renderer stderr. Security vulnerabilities belong in GitHub Security Advisories rather than a public issue.

        ## Canonical references

        - Roadmap: [Issue #34](https://github.com/iwashita-nozomu/ptymark/issues/34)
        - Adoptability ledger: [Issue #48](https://github.com/iwashita-nozomu/ptymark/issues/48)
        - Doctor v1: [Issue #43](https://github.com/iwashita-nozomu/ptymark/issues/43)
        - Next release: [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90)
        """
    ),
    "Troubleshooting-ja.md": page(
        """
        # トラブルシューティング

        [English](Troubleshooting) · [日本語](Troubleshooting-ja)

        ## 最初にdoctorを実行

        ```text
        ptymark doctor
        ptymark doctor --json
        ptymark doctor --support-report ptymark-support.json
        ptymark doctor --config /path/to/ptymark.toml
        ```

        doctor v1はdefaultでside-effect-freeです。install、download、network access、renderer/browser/PTY child起動、config/state変更を行いません。

        | Status | Exit | 意味 |
        | --- | ---: | --- |
        | ready | 0 | 選択したconfigを利用できる |
        | degraded | 10 | fallbackまたはoptional capability不足でも利用できる |
        | unusable | 20 | strict configまたは必須hostが動作できない |

        ```mermaid
        flowchart TD
            S[render/terminalの問題] --> D[ptymark doctorを実行]
            D --> R{Status}
            R -->|ready| P[対象block/presenterを確認]
            R -->|degraded| F[source/safe fallbackとremedy]
            R -->|unusable| C[config/install state/dependency/hostを修正]
            P --> J[redacted ptymark.doctor.v1 reportを添付]
            F --> J
            C --> J
        ```

        ## すぐ使える復旧mode

        ```text
        ptymark --source -- COMMAND
        ptymark --safe -- COMMAND
        ptymark --private -- COMMAND
        ptymark preview --source FILE
        ptymark preview --safe FILE
        ```

        - `--source`: supported fenceは検出するがexact sourceを表示し、外部renderer/presenterを起動しない。
        - `--safe`: semantic detectionを完全bypassし、renderer/presenter/cacheを使わない。
        - `--private`: rendering policyは維持し、自動persistent source-bearing stateを無効化する。

        ## Renderer stall/failure

        alpha.2では外部attemptを10秒、後続pending outputを1 MiBに制限します。non-strict failureではexact sourceをcommitしてから後続出力を解放します。failed/timed-out/cancelled/invalid/presentation-failed resultはcacheへ入りません。

        代表的なfinding code:

        ```text
        render.timeout
        render.output_limit
        render.process_exit
        presentation.fallback
        engine.missing
        engine.incompatible
        browser.unavailable
        presenter.unsupported
        ```

        ## Redaction保証

        default doctor/support outputは、semantic source、child environment、credential/token/cookie、command history、source-bearing raw renderer stderr、識別的path prefix、terminal-control byte、invalid byte、unbounded dataを除外・変換します。

        投稿前にreportを確認してください。ただしfull environment dumpやraw renderer stderrへ置き換えないでください。security vulnerabilityはpublic issueではなくGitHub Security Advisoryで報告します。

        ## 正本

        - ロードマップ: [Issue #34](https://github.com/iwashita-nozomu/ptymark/issues/34)
        - 利用定着度: [Issue #48](https://github.com/iwashita-nozomu/ptymark/issues/48)
        - Doctor v1: [Issue #43](https://github.com/iwashita-nozomu/ptymark/issues/43)
        - 次のリリース: [Issue #90](https://github.com/iwashita-nozomu/ptymark/issues/90)
        """
    ),
    "_Sidebar.md": page(
        """
        ## English

        - [Home](Home)
        - [Roadmap](Roadmap)
        - [Next Release](Next-Release)
        - [Adoptability](Adoptability)
        - [Release Status](Release-Status)
        - [Troubleshooting](Troubleshooting)

        ## 日本語

        - [ホーム](Home-ja)
        - [ロードマップ](Roadmap-ja)
        - [次のリリース](Next-Release-ja)
        - [利用定着度](Adoptability-ja)
        - [リリース状況](Release-Status-ja)
        - [トラブルシューティング](Troubleshooting-ja)
        """
    ),
    "_Footer.md": page(
        """
        [English Home](Home) · [日本語ホーム](Home-ja) · [Canonical Roadmap #34](https://github.com/iwashita-nozomu/ptymark/issues/34) · [Adoptability #48](https://github.com/iwashita-nozomu/ptymark/issues/48) · [Next Release #90](https://github.com/iwashita-nozomu/ptymark/issues/90)
        """
    ),
}


for directory in (WIKI, VERIFY, EVIDENCE):
    if directory.exists():
        shutil.rmtree(directory)
EVIDENCE.mkdir(parents=True)

run("git", "clone", "--quiet", WIKI_URL, str(WIKI), auth=True)
before = run("git", "rev-parse", "HEAD", cwd=WIKI)
run("git", "config", "user.name", "github-actions[bot]", cwd=WIKI)
run(
    "git",
    "config",
    "user.email",
    "41898282+github-actions[bot]@users.noreply.github.com",
    cwd=WIKI,
)

for filename, content in PAGES.items():
    (WIKI / filename).write_text(content, encoding="utf-8")

run("git", "add", *sorted(PAGES), cwd=WIKI)
run("git", "diff", "--cached", "--check", cwd=WIKI)
changed = subprocess.run(
    ("git", "diff", "--cached", "--quiet"), cwd=WIKI
).returncode != 0
if changed:
    run(
        "git",
        "commit",
        "-m",
        "docs: publish alpha.2 closeout and alpha.3 roadmap",
        cwd=WIKI,
    )
    run("git", "push", "--quiet", "origin", "HEAD", cwd=WIKI, auth=True)
after = run("git", "rev-parse", "HEAD", cwd=WIKI)

run("git", "clone", "--quiet", WIKI_URL, str(VERIFY), auth=True)
fresh = run("git", "rev-parse", "HEAD", cwd=VERIFY)
assert fresh == after, (fresh, after)

expected = set(PAGES)
actual = {path.name for path in VERIFY.glob("*.md")}
missing = sorted(expected - actual)
assert not missing, missing

pairs = {
    "Home.md": "Home-ja.md",
    "Roadmap.md": "Roadmap-ja.md",
    "Next-Release.md": "Next-Release-ja.md",
    "Adoptability.md": "Adoptability-ja.md",
    "Release-Status.md": "Release-Status-ja.md",
    "Troubleshooting.md": "Troubleshooting-ja.md",
}
for english, japanese in pairs.items():
    en_text = (VERIFY / english).read_text(encoding="utf-8")
    ja_text = (VERIFY / japanese).read_text(encoding="utf-8")
    assert japanese.removesuffix(".md") in en_text, (english, japanese)
    assert english.removesuffix(".md") in ja_text, (japanese, english)
    assert "v0.1.0-alpha.2" in en_text
    assert "v0.1.0-alpha.2" in ja_text

for filename in (
    "Home.md",
    "Home-ja.md",
    "Roadmap.md",
    "Roadmap-ja.md",
    "Next-Release.md",
    "Next-Release-ja.md",
    "Adoptability.md",
    "Adoptability-ja.md",
    "Release-Status.md",
    "Release-Status-ja.md",
    "Troubleshooting.md",
    "Troubleshooting-ja.md",
):
    text = (VERIFY / filename).read_text(encoding="utf-8")
    assert "```mermaid" in text, filename

for filename in ("Roadmap.md", "Roadmap-ja.md", "Next-Release.md", "Next-Release-ja.md"):
    assert "v0.1.0-alpha.3" in (VERIFY / filename).read_text(encoding="utf-8")

release_text = (VERIFY / "Release-Status.md").read_text(encoding="utf-8")
for digest in (
    "e0d8aba8a84161cdd652cb2227d55d6262eb6c754edb70d3f387fb50e813971b",
    "74baad0647c51257834382f126149a76e0052fb4cff331886196ef7ea37c0354",
    "2b1c561b2eb934fb5ef0861737ddf8dcb9c6d01eb934e24263d970d23f8b07d8",
):
    assert digest in release_text

summary = {
    "wiki_repository": f"{REPOSITORY}.wiki",
    "before": before,
    "after": after,
    "fresh_clone": fresh,
    "changed": changed,
    "page_count_verified": len(expected),
    "pages": sorted(expected),
    "language_pairs": pairs,
    "release": "v0.1.0-alpha.2",
    "source_commit": "c4f23c924c855094d09077c293d06b2a6157250d",
    "next_release": "v0.1.0-alpha.3",
    "next_release_issue": 90,
    "mermaid_verified": True,
    "language_switches_verified": True,
    "release_hashes_verified": True,
}
(EVIDENCE / "summary.json").write_text(
    json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
)
(EVIDENCE / "files.txt").write_text(
    "\n".join(sorted(actual)) + "\n", encoding="utf-8"
)
(EVIDENCE / "wiki-commit.txt").write_text(after + "\n", encoding="utf-8")
print(json.dumps(summary, indent=2, sort_keys=True))
