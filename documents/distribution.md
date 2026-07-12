<!--
@dependency-start
contract distribution
responsibility Defines native archive, optional renderer bundle, release workflow, checksum, and reproducibility contracts.
upstream design ./system-design.md separates binary, plugin, renderer bundle, and canonical Docker responsibilities
upstream design ./dependencies.md defines dependency and license boundaries
upstream implementation ../scripts/package-ptymark-release.sh creates the documented archive layout
downstream workflow ../.github/workflows/ptymark-release.yml builds and publishes native artifacts
@dependency-end
-->

# ptymark 配布

## 配布単位

`ptymark`は次を別々に扱います。

1. native Rust binary archive
2. WezTerm plugin source
3. optional renderer bundle
4. canonical developer／CI Docker image

native archiveは1と2に加え、README、license、検証済みconfig例、offlineで参照できるproject文書を含めます。Mermaid、MathJax、KaTeX、Puppeteer、Chromium、Typst、Node.jsは同梱しません。

## Native release artifact

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
├── plugin/
│   └── init.lua
├── examples/
│   ├── ptymark.example.toml
│   └── config/
│       ├── minimal.toml
│       ├── private.toml
│       ├── ci.toml
│       ├── wezterm-interactive.toml
│       └── custom-process.toml
└── documents/
    ├── README.md
    ├── system-design.md
    ├── design-review.md
    ├── architecture.md
    ├── renderer-architecture.md
    ├── configuration.md
    ├── ui-design.md
    ├── extension-guide.md
    ├── usage.md
    ├── dependencies.md
    ├── development-environment.md
    ├── distribution.md
    └── licensing-policy.md
```

`scripts/package-ptymark-release.sh`がarchiveとSHA-256 checksumを生成します。

```bash
cargo build --locked --release
bash scripts/package-ptymark-release.sh target/release/ptymark dist
```

package smokeはarchive内のbinary、plugin、canonical config、個別config例、文書、checksumの存在を確認します。

## Install routes

優先順:

1. GitHub Release archive
2. `cargo install --locked --git ...`
3. cloned sourceで`cargo install --locked --path .`

package manager formula、Homebrew tap、AUR、Nixは、binary interfaceとrelease layoutが安定してから追加します。

### Archive install

```bash
tar -xzf ptymark-v<VERSION>-<TARGET>.tar.gz
cd ptymark-v<VERSION>-<TARGET>
install -m 0755 ptymark "$HOME/.local/bin/ptymark"
ptymark --version
ptymark config check --config examples/config/minimal.toml
```

checksum:

```bash
sha256sum --check ptymark-v<VERSION>-<TARGET>.tar.gz.sha256
```

macOSでは`shasum -a 256`を使用できます。

## Platform matrix

初期release matrix:

| Runner | Target produced | Status |
| --- | --- | --- |
| Ubuntu x86_64 | `x86_64-unknown-linux-gnu` | native workflow |
| macOS Apple Silicon | `aarch64-apple-darwin` | native workflow when runner architecture matches |
| macOS Intel | `x86_64-apple-darwin` | planned if runner/toolchain is available |
| Windows | none | ConPTY design is outside initial scope |

Docker CIはLinuxのcanonical dependency／renderer environmentです。macOS archiveはmacOS runnerでnative buildし、Docker checksを置き換えず補完します。

## GitHub Actions

### Product CI

`.github/workflows/ptymark-ci.yml`:

- product／document path changeで起動
- canonical Docker imageをbuild
- Rust format／Clippy／library／contract tests
- runtime provider、strict process、artifact validation tests
- configuration fixture validation
- WezTerm／Bash／ShellCheck／Python／Node static checks
- Mermaid／MathJax／KaTeX／Typst correctness smoke
- real renderer／cache benchmarkとbudget gate
- Linux／macOS native buildとarchive smoke
- benchmark／native artifact upload

既存template／AgentCanon CIは削除・置換しません。

### Release

`.github/workflows/ptymark-release.yml`:

- `v*` tagとmanual dispatchで起動
- tag versionと`Cargo.toml` versionを照合
- Linux／macOS native release binaryをbuild
- packaging scriptで同じarchive／checksumを生成
- artifactをjobごとに一意名でupload
- tag eventではGitHub Releaseへattachするpublish jobを実行

release jobは`contents: write`が必要ですが、通常CIは`contents: read`に限定します。

## Optional renderer bundle

renderer backendはnative archiveと独立配布します。

想定layout:

```text
ptymark-renderers-v<VERSION>/
├── worker.mjs
├── package.json
├── package-lock.json
├── protocol.json
├── engine-inventory.json
├── THIRD_PARTY_NOTICES
└── licenses/
```

bundle requirements:

- exact npm lock integrity
- ptymark／worker protocol compatibility metadata
- engine ID／version／artifact media type inventory
- Node／Chromium support range
- platform／architecture documentation
- checksum and future signature policy
- complete third-party licenses／notices
- explicit install directory and uninstall procedure
- security update cadence independent from plain binary

normal render／child startup中にbundleをinstallしません。missing bundleはengine doctorで理由を表示し、preview／source fallbackを維持します。

## Docker image

canonical Docker imageはdeveloper／CI用です。

- Rust、Node、Chromium、Typst、Lua、fonts、ShellCheckを固定
- real renderer smoke／benchmarkを再現
- terminal blockごとにcontainerを起動するruntime dependencyではない
- production renderer bundleと同一物とはみなさない

公開imageを将来提供する場合はbase image digest、apt manifest、SBOM、vulnerability scan、retention policyを定義します。

## Versioning

alpha期間:

```text
0.1.0-alpha.1
0.1.0-alpha.2
```

次には独立versionがあります。

```text
crate／CLI version
configuration schema version
renderer worker protocol version
engine ID/version
artifact media type contract
presenter ID/version
persistent cache-key schema version
renderer bundle version
```

公開CLI、plugin option、renderer protocol、cache schemaに互換性のない変更がある間はpre-release versionを使用します。

## Release checklist

- [ ] Cargo package versionとtag一致
- [ ] `Cargo.lock`／`package-lock.json` committed
- [ ] README current-state／limitations正確
- [ ] config examplesすべて`config check`成功
- [ ] engine inventory／doctor output review
- [ ] Linux／macOS native format／Clippy／tests成功
- [ ] canonical Docker correctness／benchmark成功
- [ ] inherited repository／Docker checks成功
- [ ] archive layout smoke
- [ ] checksum verification
- [ ] renderer bundleを同梱していないこと
- [ ] third-party inventory for any separately published bundle
- [ ] WezTerm plugin smoke
- [ ] design review merge gate完了

## Reproducibility

- Rust resolution: `Cargo.lock`
- Rust compiler: `rust-toolchain.toml`
- JavaScript resolution: `renderers/package-lock.json`
- renderer pins: `docker/ptymark-versions.env`
- canonical build: `docker/ptymark.Dockerfile`
- base image digest: release candidate evidence
- native archive: `scripts/package-ptymark-release.sh`
- checksum: same script and workflow artifact
- benchmark: JSON plus budget report

release workflow、product CI、local packagingは同じscriptとlayoutを使用し、別々のarchive contractを持ちません。
