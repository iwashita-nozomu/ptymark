<!--
@dependency-start
contract design
responsibility Documents ptymark CLI, configuration, fallback, renderer adapter, and WezTerm usage.
upstream design ./architecture.md pre-display rendering behavior
upstream design ./configuration.md configuration discovery and profile behavior
upstream design ./renderer-architecture.md engine, coordinator, presenter, and cache boundaries
downstream implementation ../src/main.rs public CLI implementation
downstream test ../tests/cli_contract.rs stable CLI tests
downstream test ../tests/config_contract.rs configuration CLI tests
@dependency-end
-->

# ptymark 利用方法

## 現在利用できる入口

```text
ptymark -- COMMAND [ARG...]
ptymark preview [OPTIONS] [FILE|-]
ptymark demo [OPTIONS]
ptymark config paths
ptymark config check [CONFIG OPTIONS]
ptymark config show [CONFIG OPTIONS] [--provenance]
```

`preview`と`demo`は実装済みのpre-display renderer、terminal output gate、設定snapshot、
coordinator/cache骨格を直接使います。command modeは設定をchild起動前に検証してから透過
`exec`し、child PTY hostは後続実装です。

## Config

copyable example:

```bash
ptymark config check --config examples/ptymark.example.toml
ptymark config show --config examples/ptymark.example.toml --profile interactive
```

探索候補とtrust状態:

```bash
ptymark config paths
```

出力は概ね次の列です。

```text
origin  trust-state  present-or-missing  path
```

project直下の`.ptymark.toml`は`untrusted-project-not-loaded`として表示され、自動では読みません。
明示ファイル:

```bash
ptymark config check --config ./ptymark.toml
```

profile selection:

```bash
ptymark config show --profile private
PTYMARK_PROFILE=ci ptymark config check
```

すべての外部設定を無視してbuilt-in profileだけ使う場合:

```bash
ptymark config check --no-config
PTYMARK_NO_CONFIG=1 ptymark preview document.md
```

探索順序、schema、profile、engine、presentation、cache、diagnosticsの詳細は
[Configuration](./configuration.md)を参照してください。

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
| `--source` | semantic blockを元sourceのまま出力する |
| `--strict` | renderer failure時にfallbackせず失敗する |
| `--color` | 対応rendererでANSI色を有効化する |
| `--max-buffer-bytes N` | profileのsemantic buffer上限をcurrent commandだけoverrideする |
| `--terminal-width N` | rendererへ渡すcolumn hint |
| `--no-cache` | current commandだけcacheを無効化する |
| `--config PATH` | user configの後に明示TOMLを適用する |
| `--profile NAME` | named profileを選択する |
| `--no-config` | built-in profileだけを使う |

初期detectorが自動認識するのは、行境界で閉じた高確度blockだけです。

````markdown
```mermaid
flowchart LR
    A --> B
```

$$
\int_a^b f(x)\,dx
$$
````

インライン`$...$`、`#`見出し、箇条書きなどは、shell syntaxや金額との誤判定を
避けるため自動判定しません。`detection.mode = "off"`、`mermaid = false`、
`block_math = false`は検出を厳しくするだけで、terminal safety bypassを弱めません。

## 表示とfallback

- ordinary output: 受信順でbyte-for-byte passthrough
- ANSI/OSC/DCS/APC/CR/alternate screen: output gateからbyte-for-byte passthrough
- closed semantic block: renderer resultだけを表示
- unclosed/over-limit/unsafe block: original sourceを表示
- renderer error: defaultではoriginal sourceを表示
- strict renderer error: error終了
- bypass mode: detector保留sourceをflushした後、直接passthrough

CLIへ一度表示したsourceを後からカーソル移動で消して画像へ置換することはしません。

## Engine/coordinator/presenter

`RenderCoordinator`は次を所有します。

```text
semantic block
  -> EngineSelector
  -> EngineRegistry
  -> ArtifactCache lookup
  -> RenderEngine attempt/fallback
  -> RenderArtifact validation/cache
  -> ArtifactPresenter
  -> display bytes
```

engine codeはstdoutへ直接書かず、identity/version、format、layout sensitivityを持つ
`RenderArtifact`を返します。`ArtifactPresenter`だけがterminal capabilityを見て表示形式を
決めます。unknown terminalへ画像escapeをforceする設定はありません。

built-in selection role:

| Semantic kind | Primary | Compatibility/comparator | Final fallback |
| --- | --- | --- | --- |
| Mermaid | persistent Mermaid/Puppeteer worker | one-shot Mermaid CLI | source |
| TeX block math | persistent MathJax worker | KaTeX MathML | source |
| Typst-native | Typst CLI | none | source |

## External process adapter

library APIの`ExternalRenderer`は既存renderer processを呼びます。

protocol:

- stdin: semantic block body
- stdout: render artifact
- stderr: bounded diagnostics
- environment:
  - `PTYMARK_RENDERER_PROTOCOL=stdio-v1`
  - `PTYMARK_RENDERER_ID`
  - `PTYMARK_BLOCK_KIND`
  - `PTYMARK_SOURCE_BYTES`
  - `PTYMARK_COLOR`
  - `PTYMARK_TERMINAL_WIDTH`

adapterはtimeoutとstdout/stderr上限を強制します。custom engine設定は`program`とargv listを
使い、shell command string、pipe、redirect、command substitutionを形式として持ちません。
組版処理自体をRustで再実装しません。

## Command mode

```bash
ptymark -- bash -l
ptymark -- zsh -l
ptymark -- fish
ptymark -- codex
```

現alphaでは設定を完全に解決・検証した後、commandを同じstdin/stdout/stderr、environment、
working directoryで`exec`します。commandの終了statusは`ptymark`のstatusになります。
設定エラー時はchildを起動しません。

後続PTY hostでは同じ公開形のまま、child PTY outputだけをdisplay前pipelineへ接続します。
入力、termios、signal、resize、exit statusはrenderer configurationの対象外です。

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
| `shell` | `$SHELL` or `/bin/sh` | command modeで起動するshell |
| `login_shell` | `true` | shellへ`-l`を追加する |
| `command` | none | 完全なargument array。指定時はbinary/shellを使わない |
| `label` | `ptymark shell` | launch menu label |
| `key` | `CTRL|SHIFT+P` | key binding table。`false`で無効 |
| `launch_menu` | `true` | launch menu entry。`false`で無効 |
| `cwd` | none | spawned tab working directory |
| `set_environment_variables` | none | spawned process environment |

既存の`config.keys`と`config.launch_menu`へ追記し、既存項目を置換しません。rendering profileは
Lua側で複製せず、将来pluginから`PTYMARK_PROFILE`またはCLI selectorとして渡します。

## UI/resize/cache

現在のlibrary APIは次を提供します。

- `Viewport`
- `LayoutSensitivity`
- `resize_action`
- `ArtifactCacheKey`
- `ArtifactCache`
- `NoopArtifactCache`
- `MemoryArtifactCache`
- `CachePolicyConfig`

実PTY hostへ接続後は、uncommitted blockを最新viewportで描画し、既にtextとしてcommit済みの
scrollbackは書き換えません。live resize/image placement/persistent cacheは
[Issue #3](https://github.com/iwashita-nozomu/ptymark/issues/3)で追跡します。
