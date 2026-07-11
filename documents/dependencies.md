<!--
@dependency-start
contract environment
responsibility Defines ptymark Rust, renderer, browser, font, and optional runtime dependency ownership.
upstream design ./renderer-architecture.md engine and artifact boundaries
upstream design ./configuration.md runtime discovery and configuration boundary
upstream environment ../Cargo.toml declares direct Rust dependencies
upstream environment ../Cargo.lock pins the Rust dependency graph
upstream environment ../renderers/package.json declares direct renderer dependencies
upstream environment ../renderers/package-lock.json pins the JavaScript renderer graph
downstream environment ../docker/ptymark.Dockerfile builds the canonical validation environment
downstream test ../scripts/check-ptymark-dependencies.sh validates exact dependency alignment
@dependency-end
-->

# ptymark 依存関係

## 方針

`ptymark`固有の開発・正式検証依存は`docker/ptymark.*`に閉じ込めます。既存の
project-template / AgentCanon用Docker、Python、C++、実験環境は削除せず、ローカル作業
基盤として維持します。

`ptymark`は図や数式のlayout/typesetting engineを再実装しません。Rustコアが所有するのは
表示前ストリーム、安全分類、意味ブロック境界、engine selection、bounded execution、
artifact cache、fallback、presentation、terminalへのcommitです。

選定した役割:

- Mermaid real-time: Mermaid library/CLI + persistent Puppeteer/Chromium worker
- Mermaid compatibility/oracle: one-shot Mermaid CLI
- TeX block math real-time: MathJax + selected MathJax font package
- TeX comparator/fallback artifact: KaTeX MathML
- Typst-native input: Typst CLI（optional）
- image placement: Kitty / iTerm2 / Sixelなどのterminal protocol presenter（optional）

Rust側で追加するlibraryは責務を限定します。

- `serde`: typed configuration DTOとresolved policy serialization
- `toml`: human-authored configuration parsing/inspection

これらはterminal I/O、PTY、engine layoutを所有しません。

## 正本

| 種別 | 正本 |
| --- | --- |
| Rust direct dependency | `Cargo.toml` |
| Rust exact resolution | `Cargo.lock` |
| Rust toolchain | `rust-toolchain.toml` |
| renderer direct dependency | `renderers/package.json` |
| renderer exact resolution | `renderers/package-lock.json` |
| container/runtime pins | `docker/ptymark-versions.env` |
| image construction | `docker/ptymark.Dockerfile` |
| mount/cache/runtime | `docker/ptymark-compose.yaml` |
| consistency check | `scripts/check-ptymark-dependencies.sh` |
| renderer correctness | `scripts/check-ptymark-renderers.sh` |
| renderer performance | `scripts/benchmark-ptymark-renderers.sh` |

## 固定値

| Dependency | Pin | Purpose |
| --- | --- | --- |
| Rust | 1.97.0 | edition 2024 core, tests, release build |
| serde | Cargo-compatible `1.x`, exact in `Cargo.lock` | typed config and provenance serialization |
| toml | Cargo-compatible `0.9.x`, exact in `Cargo.lock` | strict TOML parse and effective config output |
| Node.js image | `node:24.18.0-bookworm` | persistent workers and Debian Chromium base |
| Mermaid CLI | 11.16.0 | compatibility CLI and shared renderer library surface |
| Mermaid | 11.16.0 | persistent Mermaid renderer |
| MathJax | 4.1.3 | TeX block math to SVG |
| MathJax NewCM font | 4.1.3 | deterministic MathJax SVG font data |
| KaTeX | 0.17.0 | low-latency TeX to MathML comparator |
| Puppeteer | 25.2.1 | persistent Chromium lifecycle |
| Typst CLI | 0.15.0 | explicit Typst-native source to SVG/PDF |
| Chromium | Debian bookworm package | headless Mermaid rendering |
| Lua | 5.4 | WezTerm plugin syntax and API-shape smoke |
| ShellCheck | Debian bookworm package | product shell scripts |
| Noto fonts | Debian bookworm packages | Latin, CJK, emoji renderer coverage |

Node.js patch、renderer package、browser bridgeは固定します。release candidateではCIが解決した
base image digestとDebian Chromium versionもevidenceへ記録します。

## 実行時依存

plain `ptymark` binaryは外部renderer runtimeなしでも次を提供します。

- exact source passthrough
- terminal output safety gate
- preview/source engine
- config parse/check/show
- bounded memory/no-op cache

optional backend:

- Mermaid worker: Node.js、locked renderer bundle、Chromium
- Mermaid CLI: Node.js、Mermaid CLI、Chromium
- MathJax worker: Node.js、MathJax、font package
- KaTeX comparator: Node.js、KaTeX
- Typst backend: `typst`
- WezTerm plugin: WezTermとhost OS用`ptymark` binary

通常terminal session中に`npm install`、browser download、`cargo install`を自動実行しません。
missing/incompatible engineは診断可能なunavailable状態となり、次候補またはsourceへfallback
します。dependency installationは明示的なpackage operationまたは将来のrenderer-bundle command
だけが行います。

## Workerとexternal process境界

`RenderEngine`は`RenderArtifact`を返し、terminal stdoutへ直接書きません。process adapterは
次を上限付きで扱います。

- stdin: semantic body
- stdout: artifact bytes
- stderr: diagnostics
- wall-clock timeout
- process lifecycle/cancellation
- identity/version/protocol handshake

custom engine設定は`program`とargv配列であり、shell command string、pipe、redirect、command
substitutionを持ちません。environmentは明示値またはallowlist継承に限定します。

## 更新手順

### Rust dependency

```bash
$EDITOR Cargo.toml
cargo update
cargo metadata --locked --format-version 1
```

### Renderer dependency

```bash
$EDITOR renderers/package.json
cd renderers
npm install --package-lock-only --ignore-scripts
cd ..
```

共通手順:

1. `docker/ptymark-versions.env`のpinを更新する。
2. Dockerfile/Compose fallbackと文書を同時に更新する。
3. lockfile差分、license、security advisory、transitive sizeを確認する。
4. cacheなしでcanonical imageを再構築する。
5. correctness、terminal compatibility、configuration、performance checksをGitHub Actionsで実行する。
6. benchmark artifactとengine inventoryを比較する。

```bash
docker compose \
  --env-file docker/ptymark-versions.env \
  --file docker/ptymark-compose.yaml \
  build --pull --no-cache

make ptymark-check
make ptymark-benchmark
```

ローカル値は早期feedbackです。PRの正式evidenceはGitHub Actionsのcanonical Docker job、native
Linux/macOS jobs、benchmark artifactsです。

## 検証内容

`make ptymark-check`および`.github/workflows/ptymark-ci.yml`は次を確認します。

- compiler、toolchain file、package MSRVの一致
- `cargo metadata --locked`とserde/tomlの存在
- Node/renderer package/lockfileのexact version一致
- Chromium、Typst、Lua、fonts、ShellCheckのavailability
- Rust format、Clippy、全unit/integration contract tests
- configuration parse、inheritance、unknown key、pre-launch failure
- output safety、ANSI/OSC/DCS/CR/alternate-screen byte equality
- engine registry/selector/coordinator/cache/presenter contracts
- Mermaid SVG、MathJax SVG、KaTeX MathML、Typst SVG
- persistent worker、one-shot、cache-hitのp50/p95/max
- benchmark JSON/budget artifact upload
- WezTerm plugin configuration bridge
- release archive/checksum

## ライセンスと配布

- repository本体: Apache-2.0
- Rust crates: `Cargo.lock`とrelease license inventoryで確認
- npm packages: `package-lock.json`とlicense inventoryをrelease evidenceに保存
- Chromium/apt packages: image package manifestを保存
- fontをartifactへ埋め込む場合: font license/attributionを確認

外部toolをplain binary archiveへ同梱しません。配布archiveは`ptymark` binary、README、LICENSE、
WezTerm pluginだけです。self-contained renderer bundleを提供する場合は別artifact・別inventory・
別security cadenceとします。
