# ptymark 配布

## 配布対象

`ptymark`は次を別々に扱います。

1. ネイティブRust binary
2. WezTerm plugin source (`plugin/init.lua`)
3. optional existing rendering engines
4. developer Docker image

利用者向けrelease archiveには1と2、README、LICENSEだけを含めます。
Mermaid CLI、KaTeX、Typst、Chromium、Node.jsは同梱しません。

## Release artifact

命名:

```text
ptymark-v<version>-<rust-host-target>.tar.gz
ptymark-v<version>-<rust-host-target>.tar.gz.sha256
```

archive layout:

```text
ptymark-v<version>-<target>/
├── ptymark
├── README.md
├── LICENSE
└── plugin/
    └── init.lua
```

`scripts/package-ptymark-release.sh`がarchiveとSHA-256 checksumを生成します。

```bash
cargo build --locked --release
bash scripts/package-ptymark-release.sh target/release/ptymark dist
```

## Install routes

優先順:

1. GitHub Release archive
2. `cargo install --locked --git ...`
3. cloned sourceで`cargo install --locked --path .`

package manager formula、Homebrew tap、AUR、Nixなどは、binary interfaceとrelease layoutが
安定してから追加します。

## Platform

初期release matrix:

| Runner | Target produced | Status |
| --- | --- | --- |
| Ubuntu x86_64 | `x86_64-unknown-linux-gnu` | planned by release workflow |
| macOS Apple Silicon | `aarch64-apple-darwin` | planned by release workflow |
| macOS Intel | `x86_64-apple-darwin` | planned if runner/toolchain is available |
| Windows | none | ConPTY design is out of initial scope |

Docker CIはLinuxのcanonical dependency/test environmentです。macOS artifactはmacOS runnerで
native buildし、Docker checksを置き換えず補完します。

## GitHub Actions

### Product CI

`.github/workflows/ptymark-ci.yml`:

- product path changeで起動
- canonical Docker imageをbuild
- `make ptymark-check-local`をcontainer内で実行
- unit/integration/plugin/existing-engine/release-package smokeを一つのdependency environmentで検証
- Linux/macOS native Cargo smokeを別jobで実行

既存のtemplate/AgentCanon CIは削除・置換しません。

### Release

`.github/workflows/ptymark-release.yml`:

- `v*` tagとmanual dispatchで起動
- tag versionと`Cargo.toml` versionを照合
- Linux/macOS native release binaryをbuild
- packaging scriptでarchive/checksumを生成
- artifactをjobごとに一意名でupload
- tag eventではGitHub Releaseへattachするpublish jobを実行

release jobは`contents: write`が必要ですが、通常CIは`contents: read`に限定します。

## Versioning

alpha期間:

```text
0.1.0-alpha.1
0.1.0-alpha.2
```

公開CLI、plugin option、renderer protocol、cache schemaに互換性のない変更がある間は
pre-release versionを使用します。

release前checklist:

- Cargo package version
- tag version
- README current-state記述
- `Cargo.lock`
- Docker tool pins
- architecture/UI/dependency docs
- release workflow checks
- archive contents
- checksum
- third-party license inventory
- WezTerm plugin smoke

## Existing engines

renderer backendは独立配布を基本とします。

- Mermaid CLI: user/runtimeが導入
- KaTeX: user/runtimeが導入
- Typst CLI: optional user/runtimeが導入

将来、self-contained renderer bundleを配る場合は、binary本体とは別artifactにし、size、
platform support、license、security update cadenceを明示します。

## Reproducibility

- Rust resolution: `Cargo.lock`
- Rust compiler: `rust-toolchain.toml`
- external engine pins: `docker/ptymark-versions.env`
- canonical build definition: `docker/ptymark.Dockerfile`
- base image digest: release candidateで記録
- release archive: packaging scriptから生成
- checksum: workflow artifactとして保存

release workflowとローカルpackagingは同じscriptを使い、別々のarchive layoutを持ちません。
