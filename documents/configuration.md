<!--
@dependency-start
contract design
responsibility Defines ptymark configuration discovery, schema, profiles, immutable snapshots, custom process trust, and typed runtime policy boundaries.
upstream design ./system-design.md control-plane and runtime composition model
upstream design ./renderer-architecture.md renderer coordinator and cache abstractions
upstream design ./design-review.md reviewed configuration and process-security findings
downstream implementation ../src/config.rs high-level configuration service and snapshot API
downstream implementation ../src/config/model.rs typed raw and resolved configuration models
downstream implementation ../src/config/source.rs source discovery and trust provenance
downstream implementation ../src/config/resolve.rs merge, profile inheritance, and cross-field validation
downstream implementation ../src/runtime.rs translates snapshots into runtime providers
downstream test ../tests/config_contract.rs user-facing configuration contract tests
downstream test ../tests/extension_validation_contract.rs custom engine and redaction tests
@dependency-end
-->

# ptymark configuration

## Purpose and ownership

設定ファイルが制御するのは**pre-display rendering path**だけです。

設定可能:

- explicit semantic detector
- ordered engine candidate
- artifact preference
- render timeout／worker lifecycle policy
- presentation allowlist
- bounded cache
- diagnostics／metrics
- WezTerm session profile selector

設定不可:

- keyboard input
- termios／raw mode／echo
- signal forwarding
- child exit status
- child PTY resize
- mouse／bracketed paste
- ANSI／OSC／DCS／APCのbyte-exact rule
- committed scrollbackのretroactive rewrite

terminal safetyはuser optionではなくproduct invariantです。

## Resolution lifecycle

stream loopはraw TOMLを読みません。

```text
ConfigSource[]
    ↓ discovery and trust classification
ConfigFile
    ↓ strict TOML parse / schema validation
merged ConfigFile
    ↓ profile inheritance / cross-field validation
ResolvedConfig
    ↓ explicit session overrides
ConfigSnapshot
    ├─ generation
    ├─ stable policy fingerprint
    ├─ Arc<ResolvedConfig>
    └─ Arc<ConfigProvenance>
    ↓
RuntimeBuilder
    ├─ DetectorProvider
    ├─ EngineProvider[]
    ├─ CacheProvider
    └─ PresenterProvider
```

`ConfigSnapshot`作成後、active sessionの設定は変化しません。reloadはfuture session用の新しいgenerationを作ります。snapshot fingerprintはprocess-local cache／options identity用であり、project trust用cryptographic digestではありません。

設定failureはrenderer／child process起動、future PTY raw mode移行より前に返ります。partially applied configurationはありません。

## File format

human-authored formatはTOMLです。

```toml
schema_version = 1
default_profile = "interactive"
```

`schema_version`は明示的なmigration boundaryです。unknown keyをエラーにし、typoがsilent defaultへ変わることを防ぎます。

実行可能fixture:

```text
examples/ptymark.example.toml       complete reference
examples/config/minimal.toml        minimal safe defaults
examples/config/private.toml        privacy-first
examples/config/ci.toml             deterministic logs
examples/config/wezterm-interactive.toml
examples/config/custom-process.toml
```

これらはCIで`ptymark config check`へ通します。

## Discovery and precedence

実装済みv1 priorityは低い方から:

```text
built-in defaults
    < user configuration
    < PTYMARK_CONFIG file
    < --config PATH
    < PTYMARK_PROFILE / --profile
    < current-session CLI overrides
```

### User path

Linux:

```text
$XDG_CONFIG_HOME/ptymark/config.toml
~/.config/ptymark/config.toml
```

macOS:

```text
~/Library/Application Support/ptymark/config.toml
```

Windows pathはConPTY runtime designと一緒に確定します。

### Project candidate

working directoryの`.ptymark.toml`は`config paths`へ表示しますが、自動loadしません。project fileはexternal executableやenvironmentを定義できるため、future trust storeが実装されるまで明示`--config`だけが選択方法です。

future trustは次へbindします。

```text
canonical project directory
cryptographic config digest
approval metadata
```

configがmaterially変わった場合は再承認が必要です。

### No-config semantics

- CLI `--no-config`: すべてのexternal file sourceを無効化し、`--config`との併用をエラーにする
- `PTYMARK_NO_CONFIG=1`: ambient user／environment fileを無効化する
- ambient no-config中のexplicit CLI `--config`: userが明示選択した一つのfileとして利用可能

## Built-in profiles

| Profile | Intended use |
| --- | --- |
| `interactive` | explicit Mermaid／math detection、memory cache、source fallback |
| `source` | exact source presentation、image-oriented outputなし |
| `private` | no cache、source diagnosticsなし、metricsなし |
| `ci` | deterministic source presentation、no cache、prewarmなし |

profileはzeroまたはone parentを継承します。

```toml
[profiles.my-shell]
extends = "interactive"

[profiles.my-shell.cache]
max_entries = 32
```

merge rule:

```text
scalar      child replaces parent when present
table       field-wise recursive merge
array       child replaces parent
extends     one parent only
```

cycleはfull path付きstartup errorです。

## Session override

共通CLI:

```text
--config PATH
--profile NAME
--no-config
--private
```

preview-specific:

```text
--source
--strict
--max-buffer-bytes N
--terminal-width N
--no-cache
```

nested TOML propertyすべてへCLI flagを作りません。複雑／再利用する変更はprofileへ置きます。

### Private override

`--private`またはprivate profileはsnapshot freeze前に次を強制します。

```text
cache backend = none
cache private = true
diagnostics sink = stderr
diagnostics file path = none
include_source = false
metrics = false
```

このoverrideはinput、termios、signal、resize、child environment、exit statusへ触れません。

## Detection policy

```toml
[profiles.interactive.detection]
mode = "explicit-blocks" # off | explicit-blocks
mermaid = true
block_math = true
max_buffer_bytes = 1048576
max_line_bytes = 65536

[profiles.interactive.detection.fences]
mermaid = ["mermaid"]
math = ["math", "latex", "tex"]
```

initial detectorはline-bounded explicit constructだけを扱います。

````markdown
```mermaid
A --> B
```

$$
E = mc^2
$$

```latex
E = mc^2
```
````

configでできるのはdisable、limit縮小、明示alias変更です。次のminimum safety setを弱められません。

- ANSI／OSC／DCS／APC／PM／unknown controlはbyte-exact passthrough
- alternate screen／cursor-addressed UIはbypass
- carriage-return／backspace update regionはbypass
- incomplete／oversized／binary／unsafe candidateはexact source
- committed textは後から消さない
- safety uncertaintyにstrict rendering errorを適用しない

異なるkindへ同じaliasを設定した場合、そのaliasは曖昧として検出しません。

## Engine selection policy

```toml
[profiles.interactive.engines.mermaid]
candidates = ["mermaid-worker", "mermaid-cli", "source"]
preferred_artifacts = ["image/svg+xml", "text/plain"]

[profiles.interactive.engines.math]
candidates = ["mathjax-worker", "katex", "source"]
preferred_artifacts = ["image/svg+xml", "application/mathml+xml", "text/plain"]
```

candidate orderはdeterministicです。resolverは`source`を最後のfallbackとして到達可能にします。

reserved built-in IDs:

```text
mermaid-worker
mermaid-cli
mathjax-worker
katex
typst
source
preview
```

external engineはreserved IDを再定義できません。Rust embedding applicationがbuilt-in catalogを置換する場合は、configurationではなく`RuntimeBuilder::without_engine_providers()`を明示して独自providerを登録します。

`RuntimeMode::Preview`はdependency-free確認を優先し、built-in `preview`を候補先頭へ置きます。real engine orderはembedding／future terminal runtimeで使います。

## Custom process engine

```toml
[engines.custom-mermaid]
type = "process"
version = "1"
semantic_kinds = ["mermaid"]
artifact_types = ["image/svg+xml"]
layout = "pixels"
execution = "one-shot"
program = "/opt/tools/render-mermaid"
args = ["--format", "svg"]
timeout_ms = 1500
max_stdout_bytes = 8388608
max_stderr_bytes = 65536
working_directory = "/tmp"
inherit_environment = ["PATH", "LANG"]

[engines.custom-mermaid.environment]
RENDER_MODE = "offline"
```

security/runtime rule:

- `program`はabsolute path
- optional `working_directory`もabsolute path
- shell command stringなし
- pipe、redirect、command substitutionなし
- process environmentをclear
- `inherit_environment`だけをhostから復元
- explicit environmentが継承値をoverride
- stdinはsemantic body
- stdout／stderr／timeoutを独立制限
- non-zero／empty／overflow／timeoutはfailure
- artifact identity、kind、format、layout、payloadをcache前に検証
- normal render中にinstall／downloadしない

`execution = "persistent-worker"`はschema上のstable boundaryですが、専用worker providerがない現在はruntime build errorです。one-shotへsilent downgradeしません。

## Runtime configuration

```toml
[runtimes.node]
program = "node"
required_version = ">=24.18.0 <25"
args = []

[runtimes.chromium]
program = "/usr/bin/chromium"
args = ["--headless=new"]

[renderer_bundle]
path = "/opt/ptymark-renderers"
require_lock_match = true
```

built-in runtime discoveryだけがexplicit PATH-search policyを使えます。custom engineはabsolute path policyです。

bundle discovery:

```text
renderer_bundle.path
PTYMARK_RENDERER_ROOT
/opt/ptymark-renderers when present
unavailable
```

missing optional bundleは`RuntimeBuildReport`へreasonを記録し、source／preview operationを妨げません。

## Presentation policy

```toml
[profiles.interactive.presentation]
mode = "auto" # auto | text | source
prefer = ["image/svg+xml", "application/mathml+xml", "text/plain"]
image_protocols = ["kitty", "iterm2", "sixel"]
unsupported = "source"
transparent_background = true
max_columns = 120
max_rows = 40
preserve_aspect_ratio = true
```

これはallowlistでありforce switchではありません。verified capabilityがauthoritativeです。unknown terminalへimage escapeを出しません。

current presenter:

```text
terminal/text-v1
terminal/source-v1
```

image protocol presenterはfuture providerです。

## Scheduling policy

```toml
[profiles.interactive.render]
soft_latency_budget_ms = 250
hard_timeout_ms = 1500
max_in_flight = 1
ordering = "strict"
prewarm = true
worker_idle_ms = 300000
worker_max_requests = 1000
```

current runtimeはsynchronous strict orderingです。future async schedulerはmonotonic commit sequence、bounded in-flight、bounded pending bytes、cancellation、viewport generationを同じtyped policyから実装します。

## Cache policy

```toml
[profiles.interactive.cache]
backend = "memory" # none | memory | disk | tiered
max_entries = 128
max_bytes = 33554432
ttl_seconds = 3600
private = false
```

current implementation:

```text
none    NoopArtifactCache
memory  bounded MemoryArtifactCache
```

`disk`／`tiered`はschemaとprovider boundaryを確保済みですが、provider未登録時はpre-launch runtime build errorです。

cache identity:

```text
source fingerprint
semantic kind
engine ID/version
artifact format
layout-sensitive viewport
full runtime options fingerprint
theme fingerprint
presenter ID
terminal capability fingerprint
```

persistent backendはnon-cryptographic in-memory fingerprintをそのままtrustせず、cryptographic digestとkey schema versionを追加します。

## Diagnostics

```toml
[diagnostics]
level = "warn"
format = "text"
sink = "stderr" # stderr | file | both
include_source = false
metrics = true
```

fileを含むsinkにはpathが必要です。renderer diagnosticsはdisplay stdoutへ混ぜません。source inclusionはdefault false、private modeでは強制falseです。

`config show`ではcustom environment valueを`<redacted>`にします。一方、internal fingerprint materialはredact前のpolicyを使い、異なるsecret／engine optionが同じcache identityになることを防ぎます。fingerprint materialは表示／logしません。

## Inspection commands

```bash
ptymark config paths
ptymark config check --config ./ptymark.toml
ptymark config show --profile interactive
ptymark config show --config ./ptymark.toml --provenance
ptymark engine list --profile interactive
ptymark engine doctor --config ./ptymark.toml --profile interactive
```

- `paths`: candidate、trust、present state
- `check`: parse、inheritance、cross-field validation
- `show`: effective redacted snapshot policy
- `list`: registered descriptor inventory
- `doctor`: provider、registered／unavailable engine、cache、presenter、warning

provenanceはstderrへ出し、stdoutのeffective TOMLを機械可読に保ちます。

## Validation matrix

| Invalid condition | Result |
| --- | --- |
| unknown key／syntax error | startup failure |
| unsupported schema version | startup failure |
| unknown profile／inheritance cycle | startup failure |
| line limit > total buffer | validation failure |
| soft budget > hard timeout | validation failure |
| empty candidate／unknown artifact | validation failure |
| reserved built-in external engine ID | validation failure |
| relative custom process path | runtime build failure before render/child |
| custom persistent worker without provider | runtime build failure before render/child |
| file diagnostics without path | validation failure |
| private mode with source diagnostics | resolved to safe private policy |
| disk/tiered cache without provider | runtime build failure |
| missing optional bundle | reported unavailable; fallback remains |

## Issue ownership

- [#5 configuration umbrella](https://github.com/iwashita-nozomu/ptymark/issues/5)
- [#6 discovery, precedence, provenance, and project trust](https://github.com/iwashita-nozomu/ptymark/issues/6)
- [#7 profiles, inheritance, and session overrides](https://github.com/iwashita-nozomu/ptymark/issues/7)
- [#8 detection and immutable terminal safety](https://github.com/iwashita-nozomu/ptymark/issues/8)
- [#9 engine selection and custom adapter trust](https://github.com/iwashita-nozomu/ptymark/issues/9)
- [#10 presentation and terminal capabilities](https://github.com/iwashita-nozomu/ptymark/issues/10)
- [#11 latency, cancellation, ordering, and backpressure](https://github.com/iwashita-nozomu/ptymark/issues/11)
- [#12 cache backends and privacy](https://github.com/iwashita-nozomu/ptymark/issues/12)
- [#13 diagnostics and benchmark reporting](https://github.com/iwashita-nozomu/ptymark/issues/13)
- [#14 WezTerm profile bridge](https://github.com/iwashita-nozomu/ptymark/issues/14)
- [#15 validation, editor schema, and migration](https://github.com/iwashita-nozomu/ptymark/issues/15)
- [#16 dependency provisioning and compatibility](https://github.com/iwashita-nozomu/ptymark/issues/16)
