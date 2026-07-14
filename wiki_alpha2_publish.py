#!/usr/bin/env python3
from __future__ import annotations

import datetime as dt
import json
import os
import shutil
import subprocess
from pathlib import Path

REPO = os.environ.get("GITHUB_REPOSITORY", "iwashita-nozomu/ptymark")
TOKEN = os.environ["GH_TOKEN"]
TAG = "v0.1.0-alpha.1"
NEXT = "v0.1.0-alpha.2"
ROOT = Path.cwd()
WIKI = ROOT / "wiki-alpha2"
VERIFY = ROOT / "wiki-alpha2-verify"
EVIDENCE = ROOT / "wiki-alpha2-evidence"
WIKI_URL = f"https://x-access-token:{TOKEN}@github.com/{REPO}.wiki.git"


def run(args: list[str], *, cwd: Path | None = None, capture: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        args,
        cwd=cwd,
        check=True,
        text=True,
        capture_output=capture,
        env={**os.environ, "GIT_TERMINAL_PROMPT": "0"},
    )


def gh_json(path: str) -> dict:
    result = run(["gh", "api", path], capture=True)
    return json.loads(result.stdout)


def write(path: Path, text: str) -> None:
    path.write_text(text.rstrip() + "\n", encoding="utf-8")


def language_switch(en: str, ja: str, current: str) -> str:
    if current == "en":
        return f"[English]({en}) · [日本語]({ja})"
    return f"[English]({en}) · [日本語]({ja})"


def main() -> None:
    shutil.rmtree(WIKI, ignore_errors=True)
    shutil.rmtree(VERIFY, ignore_errors=True)
    shutil.rmtree(EVIDENCE, ignore_errors=True)
    EVIDENCE.mkdir(parents=True)

    roadmap = gh_json(f"repos/{REPO}/issues/34")
    adoptability = gh_json(f"repos/{REPO}/issues/48")
    next_release = gh_json(f"repos/{REPO}/issues/66")
    current_release = gh_json(f"repos/{REPO}/releases/tags/{TAG}")
    main_ref = gh_json(f"repos/{REPO}/git/ref/heads/main")
    main_sha = main_ref["object"]["sha"]
    generated = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()

    for name, payload in {
        "roadmap-issue.json": roadmap,
        "adoptability-issue.json": adoptability,
        "next-release-issue.json": next_release,
        "current-release.json": current_release,
        "main-ref.json": main_ref,
    }.items():
        write(EVIDENCE / name, json.dumps(payload, indent=2, ensure_ascii=False))

    run(["git", "clone", WIKI_URL, str(WIKI)])

    home_en = f"""# ptymark Wiki

{language_switch('Home', 'Home-ja', 'en')}

ptymark is a pre-display renderer for explicit semantic blocks in terminal output. It preserves terminal-control traffic byte-for-byte and renders supported Mermaid or block-math input before display when a compatible engine and presenter are available.

## Project status

- Current immutable prerelease: **`{TAG}`**
- Next committed release: **`{NEXT}`**
- Next-release theme: **supportability, diagnostics, and bounded recovery**

```mermaid
flowchart LR
    A[{TAG}] --> B[#67 Re-baseline issues]
    B --> C[#13 Findings and redaction]
    C --> D[#43 Doctor v1]
    C --> E[#11 Bounded recovery]
    D --> F[#68 Support and bilingual docs]
    E --> F
    F --> G[#66 {NEXT} release]
```

## Start here

- [[Roadmap]] — canonical release order and exit gates mirrored from issue #34.
- [[Next Release|Next-Release]] — detailed `{NEXT}` scope, merge order, tests, non-goals, and publication checklist.
- [[Adoptability]] — user-journey acceptance ledger mirrored from issue #48.
- [[Release Status|Release-Status]] — current release assets and next target.
- [Repository](https://github.com/{REPO})
- [Issues](https://github.com/{REPO}/issues)
- [Releases](https://github.com/{REPO}/releases)

## Source-of-truth policy

- Issue [#34](https://github.com/{REPO}/issues/34) is the canonical roadmap.
- Issue [#66](https://github.com/{REPO}/issues/66) is the `{NEXT}` release tracker.
- Issue [#48](https://github.com/{REPO}/issues/48) is the canonical adoptability ledger.
- Wiki pages are bilingual readable mirrors. Change the issue first, then refresh the Wiki.

_Last refreshed: {generated}_
"""

    home_ja = f"""# ptymark Wiki

{language_switch('Home', 'Home-ja', 'ja')}

ptymark は、ターミナル出力に含まれる明示的なセマンティックブロックを表示前に処理するレンダラーです。ターミナル制御出力はバイト単位で維持し、対応するエンジンとプレゼンターが利用できる場合に Mermaid やブロック数式を表示前にレンダリングします。

## プロジェクト状況

- 現在の immutable prerelease: **`{TAG}`**
- 次に確定したリリース: **`{NEXT}`**
- 次版のテーマ: **サポート性、診断、上限付き復旧**

```mermaid
flowchart LR
    A[{TAG}] --> B[#67 Issue再ベースライン]
    B --> C[#13 診断Findingとredaction]
    C --> D[#43 Doctor v1]
    C --> E[#11 レンダラー停止の上限化]
    D --> F[#68 Supportと日英ドキュメント]
    E --> F
    F --> G[#66 {NEXT} リリース]
```

## 最初に読むページ

- [[ロードマップ|Roadmap-ja]] — Issue #34を正本とするリリース順序と完了条件。
- [[次のリリース|Next-Release-ja]] — `{NEXT}` の詳細スコープ、PR順、テスト、非目標、公開チェックリスト。
- [[利用定着度|Adoptability-ja]] — Issue #48を正本とする利用者受入台帳。
- [[リリース状況|Release-Status-ja]] — 現在の公開版と次版。
- [リポジトリ](https://github.com/{REPO})
- [Issues](https://github.com/{REPO}/issues)
- [Releases](https://github.com/{REPO}/releases)

## 正本の扱い

- [Issue #34](https://github.com/{REPO}/issues/34) がロードマップの正本です。
- [Issue #66](https://github.com/{REPO}/issues/66) が `{NEXT}` のリリース追跡Issueです。
- [Issue #48](https://github.com/{REPO}/issues/48) が利用者受入台帳の正本です。
- Wikiは日英の読みやすいミラーです。変更はIssueへ先に反映し、その後Wikiを更新します。

_最終更新: {generated}_
"""

    roadmap_en = f"""# {roadmap['title']}

{language_switch('Roadmap', 'Roadmap-ja', 'en')}

> Canonical source: [Issue #34]({roadmap['html_url']})  
> Next-release tracker: [Issue #66]({next_release['html_url']})  
> This page is a readable mirror. Update the canonical issue first.  
> Refreshed: {generated}

{roadmap.get('body') or ''}
"""

    roadmap_ja = f"""# ロードマップ: {NEXT} サポート性リリースと後続Train

{language_switch('Roadmap', 'Roadmap-ja', 'ja')}

> 正本: [Issue #34]({roadmap['html_url']})  
> 次版追跡: [Issue #66]({next_release['html_url']})  
> このページは日本語ミラーです。スコープや完了条件の変更は正本Issueへ先に反映します。  
> 最終更新: {generated}

## 現在の基準

`{TAG}` では、Unix PTY / Windows ConPTY、`preview`、`run -- COMMAND`、native `-- COMMAND`、source/safe/privateモード、3 OSのarchive、checksum、manifest、provenanceが公開・検証済みです。

## 次のリリース

次版は **`{NEXT}`** です。テーマは **サポート性、診断、上限付き復旧** です。

利用者が得る成果:

- 1つのdoctorコマンドで ready / degraded / unusable を判断できる。
- 公開Issueへ添付できるredacted reportを生成できる。
- config、install state、host、terminal、engine、browser、presenter、timeout、fallbackを区別できる。
- rendererが停止・過剰出力・異常終了してもexact sourceへ戻り、後続出力が順序どおり解放される。
- source/safe/privateのalpha.1契約を維持する。

## 所有Issue

| Workstream | Issue | 必須成果 |
| --- | --- | --- |
| Issue再ベースライン | #67 | #3–#22の関連Issueを「alpha.1実装済み／残作業／非所有」に整理 |
| Findingとredaction | #13 | typed findings、stable codes、共通redaction、stdout分離、private保証 |
| Doctor v1 | #43 | human/JSON/support report、`ptymark.doctor.v1`、exit 0/10/20、side-effect-free default |
| 上限付き復旧 | #11 | hard deadline、byte上限、process cleanup、順序維持source fallback |
| Supportと日英docs | #68 | issue forms、README/help、Troubleshooting日英ページ、drift test |
| リリース | #66 | version、CI、3 OS assets、checksum/manifest/provenance、独立検証 |
| 利用者受入 | #48 | merged/released evidenceのみで更新 |

## 依存関係

```mermaid
flowchart LR
    B[#67 Issue baseline] --> D[#13 Findings/redaction]
    D --> X[#43 Doctor v1]
    D --> T[#11 Bounded recovery]
    X --> S[#68 Support/docs]
    T --> S
    S --> R[#66 Release candidate]
    R --> P[Immutable {NEXT}]
    P --> V[公開asset独立検証]
    V --> U[#34/#48/Wiki closeout]
```

## 実行フェーズ

### Phase 0: Issue整理

#67を先に完了し、既に公開済みのPTY/ConPTY、session mode、archiveを将来作業として再実装しない状態にします。

### Phase 1: #13 診断基盤

- `DiagnosticFinding`相当のtyped model
- stable finding codeとseverity
- source、child environment、secret、raw renderer stderrを除外する共通redaction
- display stdoutへ内部診断を混入させない
- private modeで永続診断・metricを作らない

永続ログ、広範なmetrics、benchmark dashboardは後続版です。

### Phase 2: #43 Doctor v1

```text
ptymark doctor
ptymark doctor --json
ptymark doctor --support-report PATH
ptymark doctor --config PATH
```

```text
schema: ptymark.doctor.v1
ready: exit 0
degraded: exit 10
unusable: exit 20
```

default doctorはinstall、download、network、renderer/browser実行、PTY/ConPTY起動、config/state/cache変更を行いません。

### Phase 3: #11 上限付き復旧

- 外部render/presentation attemptにmonotonic hard deadline
- artifact/output byte上限
- pending terminal outputの既定上限 1 MiB
- timeout/overload/failure時にnon-strictではexact sourceをcommit
- `A, Bの結果/source, C` の順序を維持
- renderer/presenter processと子孫をcleanupし、利用者のPTY/ConPTY childは終了しない
- failed/timed-out/cancelled resultをcacheしない

hard timeoutの既定値は3〜15秒の範囲で現在のone-shot Mermaid/MathJax実測から決定します。

### Phase 4: #68 Support統合

- issue formが`ptymark.doctor.v1`を安全に要求
- doctor自体が起動できない場合は省略可能
- README/help/release docsを1つの診断導線へ統一
- Troubleshooting / Troubleshooting-jaを追加
- full environment dump、semantic source、raw renderer stderrを要求しない

### Phase 5: #66 リリース

- `0.1.0-alpha.2`へversion/changelog/release notesを一致
- current `main`の全gateをgreen化
- Linux x86_64、macOS aarch64、Windows x86_64を再build
- checksum、manifest、provenance付きimmutable prereleaseを公開
- 公開assetを再downloadして独立検証
- #34/#48/Wikiを公開証跡から更新

## PR順

1. #67 Issue baseline
2. #13 diagnostics/redaction PR
3. #43 doctor v1 PR
4. #11 bounded recovery PR
5. #68 support/docs/Wiki PR
6. #66 release PR

long-lived integration branchは作らず、各PRを`main`でgreenにします。

## Alpha.2の非目標

- guided setup/self-test (#44)
- CJK/grapheme/accessibility完成 (#10/#46)
- stable config v1、profile、trust、migration、routing
- upgrade/rollback/uninstall/purge (#49/#42)
- signing/notarization/Homebrew/WinGet (#51)
- persistent worker、concurrency、benchmark gate、disk cache
- Kitty/iTerm2/Sixel image placement
- 新semantic syntaxや新engine

## 後続版

- **`v0.1.0-alpha.3`（提案）**: guided setup、WezTerm導線、safe text/plain/source、CJK・accessibility、source retrieval。
- **`v0.1.0-beta.1`（提案）**: atomic upgrade、failed-upgrade recovery、offline rollback、owned-file uninstall、explicit purge。
- それ以降: stable config v1、renderer performance/persistence、capability-safe images、signed/native channels。

確定している版番号は `{NEXT}` のみです。
"""

    adoptability_en = f"""# {adoptability['title']}

{language_switch('Adoptability', 'Adoptability-ja', 'en')}

> Canonical source: [Issue #48]({adoptability['html_url']})  
> Release tracker: [Issue #66]({next_release['html_url']})  
> Check items only from merged and released evidence.  
> Refreshed: {generated}

{adoptability.get('body') or ''}
"""

    adoptability_ja = f"""# 利用定着度台帳: {NEXT} サポート性目標

{language_switch('Adoptability', 'Adoptability-ja', 'ja')}

> 正本: [Issue #48]({adoptability['html_url']})  
> リリース追跡: [Issue #66]({next_release['html_url']})  
> チェックはmerged/released evidenceがある項目だけ更新します。  
> 最終更新: {generated}

## 利用者ジャーニー

```mermaid
flowchart LR
    D[機能を知る] --> I[インストール]
    I --> R[初回render]
    R --> U[日常PTY/ConPTY利用]
    U --> X[問題の診断]
    X --> S[安全なreport共有]
    S --> C[復旧]
    C --> L[upgrade/rollback]
    L --> P[uninstall/purge]
```

## Alpha.1で達成済み

- Rust/global Nodeなしのversioned archive install
- Unix PTY / Windows ConPTY
- preview、pipe run、native session
- exact-source fallback
- source/safe/private
- 3 OS package、checksum、manifest、provenance

## Alpha.2で追加する受入条件

### 診断

- [ ] `ptymark doctor` human output
- [ ] `--json`が`ptymark.doctor.v1`
- [ ] `--support-report PATH`がdeterministic redacted artifact
- [ ] ready/degraded/unusableがexit 0/10/20
- [ ] config、install state、host、terminal、engine、browser、presenter、timeout、fallbackを区別
- [ ] 非ready findingにremedyまたはfallback理由

### 安全な共有

- [ ] semantic sourceを含まない
- [ ] child environment、history、credential/token/cookie、raw renderer stderrを含まない
- [ ] home pathとcontrol/invalid bytesを安全に処理
- [ ] private modeで永続診断・metricを自動生成しない
- [ ] issue formsがdoctor v1を要求し、doctor起動不能時は省略可能
- [ ] 日英Troubleshootingが同じredaction契約を説明

### レンダラー問題からの復旧

- [ ] 外部attemptにhard deadline
- [ ] artifact/output/pending bytesに上限
- [ ] pending既定値1 MiB
- [ ] non-strict failureはexact source
- [ ] 後続出力を順序どおり解放
- [ ] Unix/Windowsでprocess descendant cleanup
- [ ] 利用者のPTY/ConPTY childは終了しない
- [ ] failed/timed-out/cancelled resultをcacheしない

### 公開証跡

- [ ] 3 OS package-installed doctor/recovery smoke
- [ ] canonical Docker fixture
- [ ] reviewed `main`からimmutable `{NEXT}` tag
- [ ] 3 OS assets、checksums、manifest、attestations
- [ ] downloaded asset独立検証
- [ ] 公開Linux binaryのversion/doctor/source/safe smoke
- [ ] #34/#48/Wiki日英を公開証跡から更新

## Alpha.2後も未完了

- guided setup/first render
- CJK/emoji/grapheme/no-color/screen-readerの完成
- visual presentation後のsource retrieval UX
- supported upgrade/rollback/uninstall/purge
- signed package-manager channels
- stable config v1
- persistent worker/cache、image protocol

これらは未完了のままでもalpha.2を公開できます。
"""

    next_en = f"""# {next_release['title']}

{language_switch('Next-Release', 'Next-Release-ja', 'en')}

> Canonical source: [Issue #66]({next_release['html_url']})  
> Roadmap: [Issue #34]({roadmap['html_url']})  
> This page mirrors the release tracker. Update the issue first.  
> Refreshed: {generated}

{next_release.get('body') or ''}
"""

    next_ja = f"""# 次のリリース: {NEXT} — サポート性、診断、上限付き復旧

{language_switch('Next-Release', 'Next-Release-ja', 'ja')}

> 正本: [Issue #66]({next_release['html_url']})  
> ロードマップ: [Issue #34]({roadmap['html_url']})  
> このページは日本語ミラーです。変更は正本Issueへ先に反映します。  
> 最終更新: {generated}

## リリース判断

次版は **`{NEXT}`** です。alpha.1の機能範囲を広げるのではなく、既存runtimeを診断可能・support可能・停止上限付きにします。

## 利用者成果

1. ready / degraded / unusableを1コマンドで判断。
2. 公開Issueへ添付できるredacted reportを生成。
3. config、install state、host、terminal、engine、browser、presenter、timeout、fallbackを区別。
4. rendererが停止・過剰出力・異常終了してもexact sourceへ復旧。
5. 後続ターミナル出力を元の順序で継続。
6. source/safe/privateのalpha.1保証を維持。

## 依存関係

```mermaid
flowchart LR
    A[#67 Issue整理] --> B[#13 Findings/redaction]
    B --> C[#43 Doctor v1]
    B --> D[#11 Bounded recovery]
    C --> E[#68 Support/docs]
    D --> E
    E --> F[#66 Release candidate]
    F --> G[Immutable {NEXT}]
```

## 必須契約

### #13

- typed finding model
- stable finding codes
- info/warning/error severity
- 共通public-safe redaction
- display stdout分離
- private mode永続化なし

### #43

```text
ptymark doctor
ptymark doctor --json
ptymark doctor --support-report PATH
ptymark doctor --config PATH
```

```text
ptymark.doctor.v1
ready = 0
degraded = 10
unusable = 20
```

defaultはinstall/network/renderer/browser/PTY child/mutationの副作用なしです。

### #11

- monotonic hard deadline
- output/artifact byte limit
- pending output既定1 MiB
- non-strict exact-source fallback
- `A, B result/source, C` 順序維持
- renderer/presenter process descendant cleanup
- failed resultをcacheしない

hard timeoutの既定値は3〜15秒の範囲でcurrent one-shot実測から決定します。

### #68

- issue formsがdoctor v1を安全に要求
- README/help/release docsを統一
- Troubleshooting日英ページとMermaid判断図
- source、secret、full env、raw renderer stderrを要求しない

## Merge順

1. #67
2. #13 PR
3. #43 PR
4. #11 PR
5. #68 PR
6. #66 release PR

各PRを`main`へ独立mergeし、long-lived integration branchは使いません。

## リリースGate

- Rustfmt、Clippy、全Rust tests
- Ubuntu/macOS/Windows/canonical Docker
- PTY/ConPTY、managed renderer、shell/profile、WezTerm、installer、package既存gate
- doctor human/JSON/package fixtures
- redaction canary
- timeout/output/process/presenter failureと順序fixture
- Unix/Windows process cleanup
- 3 OS archive、checksum、manifest、provenance
- 公開assetの独立再検証

## 非目標

- guided setup (#44)
- CJK/accessibility完成 (#10/#46)
- config v1
- upgrade/rollback/uninstall/purge (#49)
- signing/package channels (#51)
- persistent worker/concurrency/disk cache
- terminal image protocol
- 新syntax/engine

## 完了条件

Issue #66はfeature PRのmergeでは閉じません。immutable prereleaseを公開し、downloaded assets、Linux doctor/version/source/safe、checksum、manifest、attestationを独立検証し、#34/#48/Wikiを更新して完了です。
"""

    assets = current_release.get("assets", [])
    asset_rows = "\n".join(
        f"| `{asset['name']}` | {asset.get('size', 0):,} | `{asset.get('digest') or 'n/a'}` |"
        for asset in assets
    )
    release_en = f"""# Release Status

{language_switch('Release-Status', 'Release-Status-ja', 'en')}

## Current published release

| Field | Value |
| --- | --- |
| Tag | `{current_release['tag_name']}` |
| Published | `{current_release.get('published_at') or 'n/a'}` |
| Prerelease | `{str(bool(current_release.get('prerelease'))).lower()}` |
| Current `main` at refresh | `{main_sha}` |

## Next committed release

| Field | Value |
| --- | --- |
| Version | `{NEXT}` |
| Theme | Supportability, diagnostics, and bounded recovery |
| Tracker | [Issue #66]({next_release['html_url']}) |
| Status | Planned; no tag or release yet |

```mermaid
flowchart LR
    M[Reviewed main] --> T[Immutable tag]
    T --> L[Linux build]
    T --> A[macOS build]
    T --> W[Windows build]
    L --> V[Checksums and manifest]
    A --> V
    W --> V
    V --> P[Provenance]
    P --> R[Immutable prerelease]
    R --> I[Independent downloaded-asset verification]
```

## Current assets

| Asset | Size (bytes) | GitHub digest |
| --- | ---: | --- |
{asset_rows}

The next release preserves this publication contract and adds packaged doctor/support documentation and published-binary doctor smoke tests.

_Last refreshed: {generated}_
"""

    release_ja = f"""# リリース状況

{language_switch('Release-Status', 'Release-Status-ja', 'ja')}

## 現在の公開版

| 項目 | 値 |
| --- | --- |
| Tag | `{current_release['tag_name']}` |
| 公開日時 | `{current_release.get('published_at') or 'n/a'}` |
| Prerelease | `{str(bool(current_release.get('prerelease'))).lower()}` |
| 更新時点の`main` | `{main_sha}` |

## 次に確定した版

| 項目 | 値 |
| --- | --- |
| Version | `{NEXT}` |
| テーマ | サポート性、診断、上限付き復旧 |
| Tracker | [Issue #66]({next_release['html_url']}) |
| 状態 | 計画済み。tag/releaseはまだ存在しません |

```mermaid
flowchart LR
    M[Reviewed main] --> T[Immutable tag]
    T --> L[Linux build]
    T --> A[macOS build]
    T --> W[Windows build]
    L --> V[Checksumsとmanifest]
    A --> V
    W --> V
    V --> P[Provenance]
    P --> R[Immutable prerelease]
    R --> I[公開asset独立検証]
```

## 現在のasset

| Asset | Size bytes | GitHub digest |
| --- | ---: | --- |
{asset_rows}

次版も同じ公開契約を維持し、package内doctor/support docsと公開binary doctor smokeを追加します。

_最終更新: {generated}_
"""

    sidebar = f"""**ptymark Wiki**

**English**
- [[Home]]
- [[Roadmap]]
- [[Next Release|Next-Release]]
- [[Adoptability]]
- [[Release Status|Release-Status]]

**日本語**
- [[ホーム|Home-ja]]
- [[ロードマップ|Roadmap-ja]]
- [[次のリリース|Next-Release-ja]]
- [[利用定着度|Adoptability-ja]]
- [[リリース状況|Release-Status-ja]]

**Canonical Issues**
- [#34 Roadmap](https://github.com/{REPO}/issues/34)
- [#66 {NEXT}](https://github.com/{REPO}/issues/66)
- [#48 Adoptability](https://github.com/{REPO}/issues/48)
"""

    footer = f"""[English Home](Home) · [日本語ホーム](Home-ja) · [Roadmap](Roadmap) · [ロードマップ](Roadmap-ja) · [Next Release](Next-Release) · [次のリリース](Next-Release-ja) · [Repository](https://github.com/{REPO})
"""

    pages = {
        "Home.md": home_en,
        "Home-ja.md": home_ja,
        "Roadmap.md": roadmap_en,
        "Roadmap-ja.md": roadmap_ja,
        "Adoptability.md": adoptability_en,
        "Adoptability-ja.md": adoptability_ja,
        "Next-Release.md": next_en,
        "Next-Release-ja.md": next_ja,
        "Release-Status.md": release_en,
        "Release-Status-ja.md": release_ja,
        "_Sidebar.md": sidebar,
        "_Footer.md": footer,
    }

    for name, content in pages.items():
        write(WIKI / name, content)

    run(["git", "config", "user.name", "github-actions[bot]"], cwd=WIKI)
    run(["git", "config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com"], cwd=WIKI)
    run(["git", "add", *sorted(pages)], cwd=WIKI)
    diff = run(["git", "diff", "--cached", "--stat"], cwd=WIKI, capture=True)
    write(EVIDENCE / "diff-stat.txt", diff.stdout)
    changed = run(["git", "diff", "--cached", "--quiet"], cwd=WIKI, capture=False).returncode != 0
    if changed:
        run(["git", "commit", "-m", "docs: publish alpha.2 bilingual roadmap"], cwd=WIKI)
        run(["git", "push", "origin", "HEAD:master"], cwd=WIKI)

    wiki_head = run(["git", "rev-parse", "HEAD"], cwd=WIKI, capture=True).stdout.strip()
    write(EVIDENCE / "publish-result.txt", f"changed={str(changed).lower()}\ncommit={wiki_head}")
    write(EVIDENCE / "wiki-commit.txt", run(["git", "log", "-1", "--format=fuller"], cwd=WIKI, capture=True).stdout)

    run(["git", "clone", WIKI_URL, str(VERIFY)])
    for name in pages:
        path = VERIFY / name
        if not path.is_file() or path.stat().st_size == 0:
            raise RuntimeError(f"missing or empty published page: {name}")

    required = {
        "Roadmap.md": [NEXT, "Issue #66", "```mermaid"],
        "Roadmap-ja.md": [NEXT, "Issue #66", "```mermaid"],
        "Next-Release.md": [NEXT, "ptymark.doctor.v1", "```mermaid"],
        "Next-Release-ja.md": [NEXT, "ptymark.doctor.v1", "```mermaid"],
        "Adoptability.md": [NEXT, "exit codes 0, 10, and 20", "```mermaid"],
        "Adoptability-ja.md": [NEXT, "exit 0/10/20", "```mermaid"],
        "Release-Status.md": [TAG, NEXT, "```mermaid"],
        "Release-Status-ja.md": [TAG, NEXT, "```mermaid"],
    }
    for name, needles in required.items():
        text = (VERIFY / name).read_text(encoding="utf-8")
        for needle in needles:
            if needle not in text:
                raise RuntimeError(f"{name} missing required text: {needle}")

    pairs = [
        ("Home.md", "Home-ja"),
        ("Home-ja.md", "Home"),
        ("Roadmap.md", "Roadmap-ja"),
        ("Roadmap-ja.md", "Roadmap"),
        ("Adoptability.md", "Adoptability-ja"),
        ("Adoptability-ja.md", "Adoptability"),
        ("Next-Release.md", "Next-Release-ja"),
        ("Next-Release-ja.md", "Next-Release"),
        ("Release-Status.md", "Release-Status-ja"),
        ("Release-Status-ja.md", "Release-Status"),
    ]
    for name, target in pairs:
        if target not in (VERIFY / name).read_text(encoding="utf-8"):
            raise RuntimeError(f"{name} missing language pair link to {target}")

    verified_head = run(["git", "rev-parse", "HEAD"], cwd=VERIFY, capture=True).stdout.strip()
    if verified_head != wiki_head:
        raise RuntimeError(f"wiki head mismatch: published={wiki_head} verified={verified_head}")

    summary = {
        "generated_at": generated,
        "repository": REPO,
        "current_release": TAG,
        "next_release": NEXT,
        "main_sha": main_sha,
        "wiki_head": wiki_head,
        "roadmap_issue": roadmap["number"],
        "release_tracker": next_release["number"],
        "adoptability_issue": adoptability["number"],
        "pages": sorted(pages),
    }
    write(EVIDENCE / "publication-summary.json", json.dumps(summary, indent=2, ensure_ascii=False))
    write(EVIDENCE / "verified-pages.txt", "\n".join(sorted(p.name for p in VERIFY.glob("*.md"))))
    print(json.dumps(summary, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
