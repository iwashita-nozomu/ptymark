# ptymark 依存関係

## 方針

`ptymark` 固有の開発・検証依存は `docker/ptymark.*` に閉じ込めます。
既存の project-template / AgentCanon 用 Docker、Python、C++、実験環境は削除せず、
ローカル作業基盤として維持します。

`ptymark` は図や数式の組版エンジンを再実装しません。Rust コアが所有するのは
表示前ストリーム、意味ブロック境界、既存エンジンの安全な起動、cache、fallback、
端末への commit です。

初期 backend 方針:

- Mermaid: Mermaid CLI
- Markdown / TeX block math: KaTeX
- Typst-native math/document: Typst CLI（optional backend）
- image placement: Kitty / iTerm2 / Sixel など既存 terminal protocol adapter

Rust コアは初期段階では標準ライブラリだけで構成します。PTY、ANSI parser、
外部 renderer adapter を追加するときは、公開境界、ライセンス、unsafe 使用、
feature、transitive dependency を同じ変更で記録します。

## 正本

| 種別 | 正本 |
| --- | --- |
| Rust package metadata | `Cargo.toml` |
| Rust resolution | `Cargo.lock` |
| Rust toolchain | `rust-toolchain.toml` |
| container tool pins | `docker/ptymark-versions.env` |
| image construction | `docker/ptymark.Dockerfile` |
| mount/cache/runtime | `docker/ptymark-compose.yaml` |
| consistency check | `scripts/check-ptymark-dependencies.sh` |
| external renderer smoke | `scripts/check-ptymark-renderers.sh` |

## 固定値

| Dependency | Pin | Purpose |
| --- | --- | --- |
| Rust | 1.97.0 | edition 2024 core, tests, release build |
| Node.js image | `node:24.18.0-bookworm` | Mermaid / KaTeX runtime and Debian Chromium base |
| Mermaid CLI | 11.16.0 | Mermaid source to SVG |
| KaTeX | 0.17.0 | TeX block math to HTML / MathML |
| Typst CLI | 0.15.0 | Typst-native source to SVG/PDF |
| Chromium | Debian bookworm package | headless Mermaid and HTML image rendering |
| Lua | 5.4 | WezTerm plugin syntax and API-shape smoke |
| ShellCheck | Debian bookworm package | product shell scripts |
| Noto fonts | Debian bookworm packages | Latin, CJK, emoji renderer coverage |

Node.js は LTS patch と OS family を固定します。再現性をさらに厳密にする
release candidate では、CI が解決した base image digest を `NODE_IMAGE` に記録します。

## 実行時依存

通常の `ptymark` バイナリは外部 runtime を要求しません。

- `PreviewRenderer` と `SourceRenderer`: Rust binary だけで動作
- Mermaid backend: 有効化する場合だけ `mmdc` と Chromium が必要
- KaTeX backend: 有効化する場合だけ Node.js と `katex` が必要
- Typst backend: 有効化する場合だけ `typst` が必要
- WezTerm plugin: WezTerm とホスト OS 用 `ptymark` binary が必要

`ExternalRenderer` は既存エンジン用の bounded stdio adapter です。body を stdin へ
渡し、stdout を表示用 artifact として受け取ります。renderer ID、block kind、端末幅、
色設定は環境変数でも渡します。

外部 renderer の不在、timeout、非ゼロ終了、出力上限超過の場合は既定で元ソースへ
fallback します。失敗した結果を cache しません。

## 更新手順

1. `docker/ptymark-versions.env` の値を変更する。
2. `rust-toolchain.toml` と `Cargo.toml` の Rust pin を同時にそろえる。
3. `docker/ptymark.Dockerfile` と `docker/ptymark-compose.yaml` の fallback をそろえる。
4. `cargo update` が必要な場合は `Cargo.lock` を更新する。
5. この文書の固定値とライセンス境界を更新する。
6. cache を使わず image を再構築する。
7. 全 product check を実行する。

```bash
docker compose \
  --env-file docker/ptymark-versions.env \
  --file docker/ptymark-compose.yaml \
  build --pull --no-cache

make ptymark-check
```

## 検証内容

`make ptymark-check` は Docker 内で次を確認します。

- `rustc`、Cargo、toolchain file、package MSRV の一致
- Node.js、Mermaid CLI、KaTeX、Typst、Lua、Chromium の起動
- global npm package の Mermaid CLI / KaTeX version
- Dockerfile、Compose fallback、env file の一致
- Rust format、Clippy、unit/integration tests
- `ExternalRenderer` の stdio、timeout、出力上限
- WezTerm plugin smoke
- Mermaid SVG、KaTeX MathML、Typst SVG の生成
- release archive の作成

## ライセンスと配布

- repository 本体: Apache-2.0
- Rust dependency: `Cargo.lock` と release 前の license inventory で確認
- npm dependency: Mermaid CLI、KaTeX、transitive package inventory を release evidence に保存
- apt dependency: base image/OS package manifest を release evidence に保存
- renderer が生成物へ font を埋め込む場合は font license と attribution を確認

外部ツールを binary archive に同梱しません。配布 archive は `ptymark` binary、
README、LICENSE、WezTerm plugin だけを含めます。
