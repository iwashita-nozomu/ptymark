# ptymark UI 設計

## UI の所有範囲

`ptymark` は端末エミュレータのUIを所有しません。UI面で所有するのは、
**端末へcommitする表示用バイト列と、その生成・再利用・破棄条件**です。

基本方針:

- ordinary outputは即時かつbyte-for-byteで表示する。
- semantic blockは閉じるまで保留し、閉じた時点で一つの表示単位としてcommitする。
- 一度commitしたscrollbackをカーソル移動で遡って消さない。
- text/ANSI表現を基準にし、画像はoptional backendとする。
- copy、SSH、tmux、screen、ログ保存で意味が失われないfallbackを持つ。
- 表示遅延、メモリ、外部renderer実行時間に上限を持つ。
- 図や数式の組版はMermaid CLI、KaTeX、Typstなど既存エンジンへ委譲する。

## ストリーミング時の見え方

### ordinary output

改行、ANSI、carriage returnを含め、そのままdisplay writerへ送ります。
semantic blockの前後で出力順序を変えません。

### semantic block入力中

Mermaid fenceまたは`$$` blockを検出したら、閉じる境界までsourceをbounded
bufferに保持します。初期UIではplaceholderやspinnerを先に表示しません。
後からplaceholderを消すには壊れやすいカーソル操作が必要だからです。

blockが閉じると次のどちらか一つだけをcommitします。

```text
rendered result
or
original source fallback
```

rendererがsoft latency budgetを超えた場合に、後続出力を追い越して表示する設計は
採用しません。timeoutで元ソースへ戻し、順序を維持します。

## 既存レンダリングエンジンとの関係

`ptymark`はレイアウトアルゴリズムを新規実装しません。

| Semantic kind | Primary existing engine | Artifact candidate |
| --- | --- | --- |
| Mermaid | Mermaid CLI | SVG |
| Markdown/TeX block math | KaTeX | HTML / MathML、必要ならChromium経由のimage |
| Typst-native input | Typst CLI | SVG / PDF |

Rust側の`ExternalRenderer`は共通adapterです。

- bodyをstdinへ渡す。
- terminal width、color、block kindを環境変数で渡す。
- stdoutをartifactとして受け取る。
- timeout、stdout上限、stderr上限を強制する。
- failure時はpre-display layerが元sourceへfallbackする。

既存エンジン固有のwrapperは薄いscriptまたは専用adapterに限定し、
Mermaid/TeX/Typstの構文解析・組版をRustコアへ複製しません。

## 表示サイズ

### Viewport model

`Viewport`は次を保持します。

- columns
- rows
- pixel width（取得できる場合）
- pixel height（取得できる場合）

rendererは自身の`LayoutSensitivity`を宣言します。

| Sensitivity | 再生成条件 | 主なbackend |
| --- | --- | --- |
| `Independent` | viewport changeでは再生成しない | source passthrough |
| `Columns` | column数が変わる | ANSI / Unicode text |
| `Pixels` | pixel geometryが変わる。pixel不明時はcell geometry | SVG / raster image |
| `FullViewport` | cellまたはpixel geometryのどれかが変わる | viewport-aware composite |

### resizeの扱い

将来のPTY hostは`SIGWINCH`を子PTYへ転送すると同時に、UI layerへ新しい
`Viewport`を通知します。

resize時の規則:

1. まだcommitしていないblockは最新viewportでrenderする。
2. cache keyが同じなら結果を再利用する。
3. keyが変われば再生成する。
4. 既にtextとしてscrollbackへcommitした過去blockは書き換えない。
5. 画像backendの過去block再配置は、terminal protocolがimage ID、delete、anchor、
   scroll lifecycleを安全に提供する場合だけ行う。
6. resize eventが連続する場合はruntime adapter側でdebounceし、最後のviewport
   generationだけを描画する。
7. 古いgenerationの非同期render resultはcommitせず破棄する。

現在は`resize_action`と`LayoutSensitivity`を実装・単体テストしています。
PTY resize eventと実レンダーキューへの接続はfollow-up issueで管理します。

## Render cache

### cache key

`RenderKey`は少なくとも次を含みます。

- semantic sourceの安定fingerprint
- block kind
- renderer ID / version
- layout sensitivityに応じたviewport geometry
- theme fingerprint
- renderer option fingerprint

同じsourceでも幅、テーマ、renderer version、optionが異なれば別entryです。

### memory cache

現在実装する`RenderCache`はprocess-localのbounded LRU cacheです。

既定値:

- 最大128 entries
- 最大32 MiB
- 一つのentryが全byte budgetを超える場合は格納しない

挙動:

- hitはLRU orderの末尾へ移動する。
- entry countまたはbyte budgetを超えると古いentryから削除する。
- themeとrenderer単位で明示invalidateできる。
- renderer failure、timeout、cancelled resultはcacheしない。
- source text、rendered image、診断に秘密情報が含まれ得るためdiskへ自動保存しない。

### disk cache

初期リリースでは実装しません。追加する場合は次を満たします。

- opt-in
- XDG cache / OS cache directoryを使用
- atomic writeとcrash-safe index
- total bytes、entry count、ageの上限
- renderer versionとschema versionによるinvalidation
- clear/status command
- private modeで完全無効化
- source bodyをそのままfilenameやlogへ出さない

## Theme

terminal background/foreground、dark/light、color enable、font scaleをrenderer optionまたは
theme fingerprintへ正規化します。

- ANSI rendererはterminalの既定色を優先する。
- SVG/image rendererは透明背景を既定とする。
- 透明背景が安全でないengineでは、検出した背景色を明示する。
- theme変化後は該当theme generationのcacheをinvalidateする。
- themeを取得できない場合はneutral/default theme keyを使う。

## 画像backend

画像はoptionalです。backend候補はKitty Graphics、iTerm2 Inline Images、Sixelです。

必要な責務:

- capability detection
- artifact format negotiation
- cell/pixel sizeの決定
- image ID、delete、anchor、scroll lifecycle
- resize時の再配置・再生成
- tmux/SSHを含むtransport可否
- source copy/fallbackへの到達経路

capabilityが不明、transportが壊れる、または配置管理に失敗した場合はtext/ANSIまたは
source fallbackへ戻ります。

## 表示失敗と診断

通常利用ではrenderer errorをstdoutへ混ぜません。

- stdout: original sourceまたはrendered artifact
- stderr / structured log: engine ID、duration、timeout、exit status、fallback reason
- source body: defaultではlogへ記録しない

`--strict`はテスト・開発用で、renderer failureを終了エラーとして表面化します。

## Accessibilityとcopy

- 色だけで意味を区別しない。
- ANSI rendererは`NO_COLOR`と設定を尊重する。
- image backendにはsourceを取得できるcache/session entryを残す。
- hyperlinkやOSC 8など既存terminal semanticsをpassthrough時に壊さない。
- CJK/emoji幅はterminal cell width-awareな既存libraryを採用して扱う。

## 今回の実装範囲

- `Viewport`
- `LayoutSensitivity`
- `resize_action`
- viewport/theme/renderer optionを含む`RenderKey`
- entry count/byte budgetを持つLRU`RenderCache`
- theme/renderer invalidation
- hit/miss/eviction/rejection stats
- 単体テスト

以下はPTY hostとterminal protocol adapterが必要なため、Issueで追跡します。

- live `SIGWINCH`接続とdebounce/cancellation
- image placement lifecycle
- disk cache
- WezTermからのtheme/cell pixel metrics伝達
- scrollback source retrieval UI
