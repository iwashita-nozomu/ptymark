<!--
@dependency-start
contract environment
responsibility Defines ptymark Rust, renderer, browser, font, optional runtime, and process-isolation dependency ownership.
upstream design ./system-design.md runtime composition and security model
upstream design ./renderer-architecture.md engine and artifact boundaries
upstream design ./configuration.md runtime discovery and configuration boundary
upstream environment ../Cargo.toml declares direct Rust dependencies
upstream environment ../Cargo.lock pins the Rust dependency graph
upstream environment ../renderers/package.json declares direct renderer dependencies
upstream environment ../renderers/package-lock.json pins the JavaScript renderer graph
downstream environment ../docker/ptymark.Dockerfile builds the canonical validation environment
downstream implementation ../src/runtime.rs discovers and registers optional renderer bundles
downstream implementation ../src/process_engine.rs enforces process/environment bounds
downstream test ../scripts/check-ptymark-dependencies.sh validates exact dependency alignment
@dependency-end
-->

# ptymark 依存関係

## 方針

`ptymark`固有の開発・正式検証依存は`docker/ptymark.*`に閉じ込めます。既存のproject-template／AgentCanon用Docker、Python、C++、実験環境は削除せず、ローカル作業基盤として維持します。

`ptymark`は図や数式のlayout／typesetting engineを再実装しません。Rustコアが所有するのは表示前ストリーム、安全分類、意味ブロック境界、engine selection、bounded execution、artifact validation、cache、fallback、presentation、terminalへのcommitです。

選定した役割:

- Mermaid target real-time path: Mermaid library／CLI + persistent Puppeteer／Chromium worker
- Mermaid current compatibility path: bounded one-shot Node stdio-v1
- TeX target real-time path: MathJax + NewCM font persistent worker
- TeX current compatibility/comparator path: one-shot MathJax SVG／KaTeX MathML
- Typst-native input: Typst CLI（optional）
- image placement: Kitty／iTerm2／Sixel terminal protocol presenter（optional）

persistent Rust worker clientは後続実装です。現在のrenderer-bundle providerは、同じengine implementationをversioned raw stdio-v1で一回起動する互換経路を登録します。この区別を`engine doctor`のwarningに表示します。

## Rust dependency boundary

Rust側のdirect libraryは責務を限定します。

| Crate | Responsibility | Must not own |
| --- | --- | --- |
| `serde` | typed configuration DTO、resolved policy、provenance serialization | PTY、terminal I/O、engine layout |
| `toml` | strict human-authored configuration parse、effective configuration output | runtime process execution、presentation |

process management、cache、terminal safety、provider compositionは現在standard libraryで実装しています。PTY／async runtime等の依存を追加する場合は、公開boundary、unsafe、license、feature、transitive sizeを同じ変更でレビューします。

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
| dependency consistency | `scripts/check-ptymark-dependencies.sh` |
| renderer correctness | `scripts/check-ptymark-renderers.sh` |
| renderer performance | `scripts/benchmark-ptymark-renderers.sh` |

versionを文書やDockerfileへ重複記載する場合は、consistency scriptが正本との一致を検証します。

## 固定値

| Dependency | Pin | Purpose |
| --- | --- | --- |
| Rust | 1.97.0 | edition 2024 core、tests、release build |
| serde | Cargo-compatible `1.x`, exact in `Cargo.lock` | typed config and provenance serialization |
| toml | Cargo-compatible `0.9.x`, exact in `Cargo.lock` | strict TOML parse and effective config output |
| Node.js image | `node:24.18.0-bookworm` | renderer worker／stdio process and Debian Chromium base |
| Mermaid CLI | 11.16.0 | Mermaid package surface and compatibility rendering |
| Mermaid | 11.16.0 | Mermaid source to SVG |
| MathJax | 4.1.3 | TeX block math to SVG |
| MathJax NewCM font | 4.1.3 | deterministic MathJax SVG font data |
| KaTeX | 0.17.0 | TeX to MathML comparator |
| Puppeteer | 25.2.1 | Chromium lifecycle |
| Typst CLI | 0.15.0 | explicit Typst-native source to SVG/PDF |
| Chromium | Debian bookworm package | headless Mermaid rendering |
| Lua | 5.4 | WezTerm plugin syntax and API-shape smoke |
| ShellCheck | Debian bookworm package | product shell scripts |
| Noto fonts | Debian bookworm packages | Latin、CJK、emoji renderer coverage |

Node.js patch、renderer packages、browser bridgeは固定します。release candidateではCIが解決したbase image digest、Debian Chromium version、apt package manifestもevidenceへ記録します。

## 配布単位

依存は四つの配布単位に分けます。

```text
plain Rust binary
WezTerm Lua plugin
optional renderer bundle
canonical developer/CI Docker image
```

### Plain binary

外部renderer runtimeなしでも次を提供します。

- exact source passthrough
- terminal output safety gate
- preview／source engine
- config paths／check／show
- engine list／doctor
- runtime provider composition
- bounded memory／no-op cache
- custom absolute-path one-shot process engine

### WezTerm plugin

Lua pluginはhost-native binaryを起動するだけです。Node／ChromiumをLua側へ埋め込みません。

### Renderer bundle

optional bundleは次を含む別artifactとして設計します。

- `worker.mjs`
- exact `package-lock.json` resolution
- protocol／engine inventory metadata
- licenses and notices
- integrity metadata
- documented install/uninstall path

plain binary archiveへ暗黙に同梱しません。

### Canonical Docker image

正式開発／CI用です。terminal blockごとにDockerを起動するruntime architectureではありません。

## Runtime discovery

renderer bundle探索順:

```text
[renderer_bundle].path
PTYMARK_RENDERER_ROOT
/opt/ptymark-renderers when present
unavailable
```

Node runtimeは設定された`runtimes.node.program`を使い、未指定時は`node`を明示的なPATH-search policyで探します。user-configured custom engineはabsolute path policyです。

missing／incompatible optional engineは`RuntimeBuildReport`へreason付きで記録し、`engine doctor`が表示します。source-only operationは外部dependencyなしで維持します。

## No surprise installation

通常session／render中に次を実行しません。

```text
npm install
browser download
cargo install
apt install
remote font/icon fetch
```

installationは明示的なpackage operationまたは将来のrenderer-bundle commandだけが行います。導入後はoffline operationを可能にする設計です。

## Worker and process boundary

`RenderEngine`は`RenderArtifact`を返し、terminal stdoutへ直接書きません。

raw stdio-v1:

```text
stdin   semantic body
stdout  one artifact
stderr  bounded diagnostics
```

host protocol variables:

```text
PTYMARK_RENDERER_PROTOCOL=stdio-v1
PTYMARK_RENDERER_ID
PTYMARK_BLOCK_KIND
PTYMARK_SOURCE_BYTES
PTYMARK_COLOR
PTYMARK_TERMINAL_WIDTH
```

### Strict `ProcessEngine`

user configurationから作るprocessは次を強制します。

- shellなしの`program` + argv
- absolute executable path
- absolute working directory when supplied
- environment clear
- allowlist継承
- explicit environment override
- wall-clock timeout
- independent stdout／stderr bounds
- Unix process-group termination on timeout
- non-zero／empty／overflow result rejection
- artifact identity／kind／format／layout／payload validation before cache

custom engine形式はshell command string、pipe、redirect、command substitutionを持ちません。

### Persistent target contract

persistent clientは次をversion handshakeします。

```text
ptymark protocol version
worker protocol version
engine ID/version
artifact schema/media type
runtime versions
```

さらにstartup timeout、idle lifetime、max requests、recycle、cancellation、stale viewport generationを所有します。one-shot providerから独立して差し替え可能にします。

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

1. direct declarationとlockfileを更新する。
2. `docker/ptymark-versions.env`のpinを更新する。
3. Dockerfile／Compose fallbackと文書を同時に更新する。
4. license、security advisory、transitive size、browser compatibilityを確認する。
5. cacheなしでcanonical imageを再構築する。
6. correctness、terminal compatibility、configuration、runtime、performance checksをGitHub Actionsで実行する。
7. benchmark artifactと`engine doctor` inventoryを比較する。

```bash
docker compose \
  --env-file docker/ptymark-versions.env \
  --file docker/ptymark-compose.yaml \
  build --pull --no-cache

make ptymark-check
make ptymark-benchmark
```

ローカル値は早期feedbackです。PRの正式evidenceはGitHub Actionsのcanonical Docker job、native Linux／macOS jobs、benchmark artifactsです。

## 検証内容

`make ptymark-check`および`.github/workflows/ptymark-ci.yml`は次を確認します。

- compiler、toolchain file、package MSRVの一致
- `cargo metadata --locked`とserde／tomlの存在
- Node／renderer package／lockfileのexact version一致
- Chromium、Typst、Lua、fonts、ShellCheckのavailability
- Rust format、Clippy、全unit／integration contract tests
- configuration parse、inheritance、unknown key、pre-launch failure
- output safety、ANSI／OSC／DCS／CR／alternate-screen byte equality
- runtime provider／registry／selector／coordinator／cache／presenter contracts
- strict process environment／timeout／output bounds
- artifact identity／kind／format／payload rejection
- Mermaid SVG、MathJax SVG、KaTeX MathML、Typst SVG
- raw stdio-v1 Mermaid／KaTeX adapters
- persistent Node benchmark、one-shot benchmark、Rust cache-hit p50／p95／max
- benchmark JSON／budget artifact upload
- WezTerm plugin configuration bridge
- release archive／checksum

## ライセンスと配布

- repository本体: Apache-2.0
- Rust crates: `Cargo.lock`とrelease license inventoryで確認
- npm packages: `package-lock.json`とlicense inventoryをrelease evidenceに保存
- Chromium／apt packages: image package manifestを保存
- fontをartifactへ埋め込む場合: font license／attributionを確認

外部toolをplain binary archiveへ同梱しません。self-contained renderer bundleを提供する場合は別artifact、別inventory、別integrity policy、別security cadenceとします。
