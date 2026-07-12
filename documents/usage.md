<!--
@dependency-start
contract design
responsibility Documents ptymark CLI, configuration, fallback, runtime composition, process engine protocol, and WezTerm usage.
upstream design ./system-design.md runtime planes and session lifecycle
upstream design ./configuration.md configuration discovery and profile behavior
upstream design ./renderer-architecture.md engine, coordinator, presenter, and cache boundaries
upstream design ./extension-guide.md provider extension procedures
downstream implementation ../src/cli.rs public CLI implementation
downstream implementation ../src/runtime.rs runtime composition root
downstream implementation ../src/process_engine.rs strict process adapter
downstream test ../tests/cli_contract.rs stable CLI tests
downstream test ../tests/runtime_contract.rs runtime provider tests
@dependency-end
-->

# ptymark 利用方法

READMEは利用開始とtroubleshootingを中心に説明します。この文書はCLI、runtime composition、process protocolを実装契約に近い粒度で記録します。

## 現在利用できる入口

```text
ptymark [CONFIG OPTIONS] -- COMMAND [ARG...]
ptymark [CONFIG OPTIONS] preview [OPTIONS] [FILE|-]
ptymark [CONFIG OPTIONS] demo [OPTIONS]
ptymark config paths [CONFIG OPTIONS]
ptymark config check [CONFIG OPTIONS]
ptymark config show [CONFIG OPTIONS] [--provenance]
ptymark engine list [CONFIG OPTIONS]
ptymark engine doctor [CONFIG OPTIONS]
```

`preview`と`demo`は、設定snapshotから`RuntimeBuilder`で組み立てたpre-display runtimeを使います。command modeは設定をchild起動前に検証してから透過`exec`します。child PTY hostは後続実装です。

## Session construction

CLIはrender componentを個別にnewしません。

```text
ConfigManager
  → LoadedConfig
  → apply session overrides
  → ConfigSnapshot(generation, fingerprint)
  → RuntimeBuilder
       ├─ DetectorProvider
       ├─ EngineProvider[]
       ├─ CacheProvider
       └─ PresenterProvider
  → SessionRuntime
```

`ConfigSnapshot`作成後、active sessionの設定は変化しません。reloadはfuture session用の新しいgenerationを作ります。

## Config commands

### `config paths`

```bash
ptymark config paths
```

出力列:

```text
origin  trust-state  present-or-missing  path
```

project直下の`.ptymark.toml`は`untrusted-project-not-loaded`として表示され、自動では読みません。

### `config check`

```bash
ptymark config check --config examples/ptymark.example.toml
```

次をchild／renderer起動前に検証します。

- TOML syntaxとunknown key
- `schema_version`
- profile存在、単一継承、cycle
- detector byte limit
- render timeoutとordering
- engine candidate／artifact type
- cache backend／privacy
- presentation protocol
- custom process field
- diagnostics sink／path

### `config show`

```bash
ptymark config show --config examples/ptymark.example.toml --profile interactive
ptymark config show --profile private --provenance
```

stdoutはeffective TOMLです。`--provenance`はstderrへ出します。custom engine environment valueはredactされます。

### 外部設定を無効にする

```bash
ptymark config check --no-config
PTYMARK_NO_CONFIG=1 ptymark preview document.md
```

CLI `--no-config`は`--config`と併用できません。ambient `PTYMARK_NO_CONFIG=1`中でも、CLIで明示した`--config`は使用できます。

## Preview

標準入力:

```bash
printf '%s\n' '$$' 'E = mc^2' '$$' | ptymark preview
```

file:

```bash
ptymark preview notes.md
```

built-in demo:

```bash
ptymark demo
ptymark demo --color
ptymark demo --profile private --no-cache
```

options:

| Option | Meaning |
| --- | --- |
| `--source` | semantic blockをexact sourceで出力する |
| `--strict` | renderer failure時にsource fallbackせず失敗する |
| `--color` | 対応previewでANSI色を有効化する |
| `--max-buffer-bytes N` | current sessionのsemantic buffer上限をoverrideする |
| `--terminal-width N` | rendererへ渡すcolumn hint |
| `--no-cache` | current sessionだけcacheを無効化する |
| `--config PATH` | user configの後に明示TOMLを適用する |
| `--profile NAME` | named profileを選択する |
| `--no-config` | built-in profileだけを使う |
| `--private` | no-cache／redacted sessionを強制する |

一般interactive detectorが認識するのは、行境界で閉じた高確度blockだけです。

````markdown
```mermaid
flowchart LR
    A --> B
```

$$
\int_a^b f(x)\,dx
$$

```latex
E = mc^2
```
````

inline `$...$`、heading、list markerは自動判定しません。`detection.mode = "off"`、`mermaid = false`、`block_math = false`は検出を厳しくするだけで、terminal safety bypassを弱めません。

## 表示とfallback

- ordinary safe text: 受信順でbyte-for-byte passthrough
- ANSI／OSC／DCS／APC／CR／alternate screen: output gateからbyte-for-byte passthrough
- closed semantic block: compatible rendered resultだけを表示
- unclosed／over-limit／unsafe block: exact sourceを表示
- renderer error: defaultではexact sourceを表示
- strict renderer error: error終了
- transform→bypass: detector保留sourceをflushしてから直接passthrough

表示済みsourceをcursor移動で消して後から画像へ置換しません。

## Engine inventory and doctor

### `engine list`

```bash
ptymark engine list --profile interactive
```

登録済みdescriptorをtab区切りで表示します。

```text
id  version  semantic-kinds  artifact-formats  execution-model
```

descriptorはrender前に取得でき、engine ID／versionはcache identityへ入ります。

### `engine doctor`

```bash
ptymark engine doctor --config ./ptymark.toml --profile interactive
```

診断対象:

```text
snapshot generation/fingerprint
selected profile
engine provider list
registered engine descriptor
optional unavailable engine and reason
selected cache backend
selected presenter
capability/runtime warnings
```

optional renderer bundleが無い場合でも、`preview`と`source`が登録されていればdoctorは成功します。

## Runtime coordinator

`RenderCoordinator`は一つのsemantic blockについて一つのtransactionを所有します。

```text
SemanticBlock
  → EngineSelector ordered candidates
  → EngineRegistry descriptor
  → ArtifactCache lookup
  → RenderEngine bounded attempt
  → artifact identity/kind/format validation
  → cache admission
  → ArtifactPresenter
  → display commit
```

failed、partial、timeout、overflow artifactをcache／displayしません。candidateが全失敗した場合、`PreDisplayRenderer`がprofileに従いsource fallbackまたはstrict errorを選びます。

## Built-in engines and bundle roles

| ID | Kind | Artifact | Runtime status |
| --- | --- | --- | --- |
| `preview` | math、mermaid | terminal text | built-in in-process |
| `source` | math、mermaid | source | built-in in-process |
| `mermaid-worker` | mermaid | SVG | bundle one-shot compatibility transport |
| `mermaid-cli` | mermaid | SVG | bundle one-shot stdio-v1 |
| `mathjax-worker` | math | SVG | bundle one-shot compatibility transport |
| `katex` | math | MathML | bundle one-shot stdio-v1 |

`worker`名のpersistent executionはtarget architectureを示します。現在のRust bundle providerはbounded one-shot transportを使用し、persistent client／recycle lifecycleは別providerとして後続実装します。

`RuntimeMode::Preview`はdependency-freeな`preview`を候補の先頭へ置きます。設定したreal engine orderはterminal runtime／embedding runtime／future PTY hostで利用します。bundle correctnessはDocker smokeで確認します。

## Strict process engine

`ProcessEngine`はuser-configured processをshellなしで実行します。

### Protocol

```text
stdin   exact semantic body
stdout  one artifact
stderr  bounded diagnostics
```

host environment:

```text
PTYMARK_RENDERER_PROTOCOL=stdio-v1
PTYMARK_RENDERER_ID
PTYMARK_BLOCK_KIND
PTYMARK_SOURCE_BYTES
PTYMARK_COLOR
PTYMARK_TERMINAL_WIDTH
```

### Environment isolation

process作成時にenvironmentをclearします。

```text
clear all inherited variables
  → restore only inherit_environment allowlist
  → apply explicit environment table
  → apply ptymark protocol variables
```

child command environmentとrenderer environmentは別contractです。

### Path policy

user-configured engineでは:

- `program`はabsolute path
- `working_directory`を指定する場合もabsolute path
- argvはarray
- shell expansion／pipe／redirect／substitutionなし

built-in runtime discoveryだけが明示的に`PATH` searchを選べます。

### Bounds and failure

- timeout > 0
- stdout limit > 0
- stderr limit > 0
- stdout／stderrを並行drain
- timeout時にUnix process groupをterminate
- non-zero exit、empty output、overflowはfailure

legacy `ExternalRenderer`はlow-level compatibility APIです。user configurationから構成するruntimeはstrict `ProcessEngine`を使います。

## Custom engine configuration

```toml
[engines.custom-math]
type = "process"
version = "1"
semantic_kinds = ["math"]
artifact_types = ["image/svg+xml"]
layout = "columns"
execution = "one-shot"
program = "/opt/renderers/math-svg"
args = ["--display"]
timeout_ms = 1500
max_stdout_bytes = 8388608
max_stderr_bytes = 65536
working_directory = "/tmp"
inherit_environment = ["PATH", "LANG"]

[engines.custom-math.environment]
RENDER_MODE = "offline"

[profiles.custom.engines.math]
candidates = ["custom-math", "source"]
preferred_artifacts = ["image/svg+xml", "text/plain"]
```

`execution = "persistent-worker"`を設定したcustom engineは、専用worker providerがない現在はpre-launch build errorです。one-shotと偽って実行しません。

## Command mode

```bash
ptymark -- bash -l
ptymark -- zsh -l
ptymark -- fish
ptymark -- codex
```

現alphaでは設定を完全に解決・検証した後、commandを同じstdin/stdout/stderr、environment、working directoryで`exec`します。commandの終了statusは`ptymark`のstatusになります。設定エラー時はchildを起動しません。

後続PTY hostでは同じ公開形のままchild PTY outputだけをdisplay前pipelineへ接続します。入力、termios、signal、resize、exit statusはrenderer configurationの対象外です。

## WezTerm

### Git URL

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

local ptymark = wezterm.plugin.require(
  'https://github.com/iwashita-nozomu/ptymark'
)

ptymark.apply_to_config(config, {
  binary = 'ptymark',
  config_file = '/home/user/.config/ptymark/config.toml',
  profile = 'interactive',
})

return config
```

### Local development URL

```lua
local ptymark = wezterm.plugin.require(
  'file:///absolute/path/to/ptymark'
)
```

### Options

| Option | Default | Meaning |
| --- | --- | --- |
| `binary` | `ptymark` | binary path |
| `config_file` | none | `--config PATH` |
| `profile` | none | `--profile NAME` |
| `no_config` | false | `--no-config` |
| `private` | false | `--private` |
| `shell` | `$SHELL` or `/bin/sh` | command modeで起動するshell |
| `login_shell` | `true` | shellへ`-l`を追加する |
| `command` | none | 完全なargument array。指定時はhelper optionを使わない |
| `label` | `ptymark shell` | launch menu label |
| `key` | `CTRL|SHIFT+P` | key binding table。`false`で無効 |
| `launch_menu` | `true` | launch menu entry。`false`で無効 |
| `cwd` | none | spawned tab working directory |
| `set_environment_variables` | none | spawned process environment |

既存の`config.keys`と`config.launch_menu`へ追記し、既存項目を置換しません。rendering schemaはLua側へ複製しません。

## Runtime extension API

現在のlibrary APIは次のproviderを公開します。

```text
EngineProvider
DetectorProvider
CacheProvider
PresenterProvider
```

composition entrypoint:

```rust
let runtime = RuntimeBuilder::default()
    .with_engine_provider(my_provider)?
    .build(snapshot, RuntimeRequest::preview())?;
```

duplicate provider IDとduplicate engine IDはエラーです。silent replacementは行いません。

詳細手順は[Extension Guide](./extension-guide.md)を参照してください。

## UI／resize／cache

現在のlibrary API:

- `Viewport`
- `LayoutSensitivity`
- `resize_action`
- `ArtifactCacheKey`
- `ArtifactCache`
- `NoopArtifactCache`
- `MemoryArtifactCache`
- `CachePolicyConfig`
- `ConfigSnapshot`
- `RuntimeBuilder`

実PTY host接続後はuncommitted blockを最新viewportで描画し、既にtextとしてcommit済みのscrollbackは書き換えません。live resize、generation cancellation、image placement、persistent cacheは[Issue #3](https://github.com/iwashita-nozomu/ptymark/issues/3)で追跡します。
