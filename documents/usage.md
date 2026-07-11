# ptymark 利用方法

## 現在利用できる入口

```text
ptymark -- COMMAND [ARG...]
ptymark preview [OPTIONS] [FILE|-]
ptymark demo [OPTIONS]
```

`preview`と`demo`は実装済みのpre-display rendererを直接使います。
command modeは現在は透過`exec`で、child PTY hostは後続実装です。

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
```

options:

| Option | Meaning |
| --- | --- |
| `--source` | semantic blockを元sourceのまま出力する |
| `--strict` | renderer failure時にfallbackせず失敗する |
| `--color` |対応rendererでANSI色を有効化する |
| `--max-buffer-bytes N` | detectorが保持する最大semantic buffer |
| `--terminal-width N` | rendererへ渡すcolumn hint |

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
避けるため自動判定しません。

## 表示とfallback

- ordinary output: 受信順でbyte-for-byte passthrough
- closed semantic block: renderer resultだけを表示
- unclosed/over-limit block: original sourceを表示
- renderer error: defaultではoriginal sourceを表示
- strict renderer error: error終了
- bypass mode: detector保留sourceをflushした後、直接passthrough

CLIへ一度表示したsourceを後からカーソル移動で消して画像へ置換することはしません。

## Existing engine adapter

library APIの`ExternalRenderer`は既存renderer processを呼びます。

protocol:

- stdin: semantic block body
- stdout: terminal/display artifact
- stderr: bounded diagnostics
- environment:
  - `PTYMARK_RENDERER_PROTOCOL=stdio-v1`
  - `PTYMARK_RENDERER_ID`
  - `PTYMARK_BLOCK_KIND`
  - `PTYMARK_SOURCE_BYTES`
  - `PTYMARK_COLOR`
  - `PTYMARK_TERMINAL_WIDTH`

adapterはtimeoutとstdout上限を強制します。Mermaid CLI、KaTeX、Typst固有wrapperは
このprotocolへ適合させ、組版処理自体をRustで再実装しません。

## Command mode

```bash
ptymark -- bash -l
ptymark -- zsh -l
ptymark -- fish
ptymark -- codex
```

現alphaではcommandを同じstdin/stdout/stderr、environment、working directoryで
`exec`します。commandの終了statusは`ptymark`のstatusになります。

後続PTY hostでは同じ公開形のまま、child PTY outputをdisplay前pipelineへ接続します。

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

既存の`config.keys`と`config.launch_menu`へ追記し、既存項目を置換しません。

## UI/resize/cache

現在のlibrary APIは次を提供します。

- `Viewport`
- `LayoutSensitivity`
- `resize_action`
- `RenderKey`
- `RenderCache`

実PTY hostへ接続後は、uncommitted blockを最新viewportで描画し、既にtextとして
commit済みのscrollbackは書き換えません。live resize/image placement/persistent cacheは
[Issue #3](https://github.com/iwashita-nozomu/ptymark/issues/3)で追跡します。
