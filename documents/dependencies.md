# ptymark 依存関係

## 方針

`ptymark` 固有の開発・検証依存は `docker/ptymark.*` に閉じ込めます。
既存の project-template / AgentCanon 用 Docker、Python、C++、実験環境は削除せず、
ローカル作業基盤として維持します。

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
| Node.js image | `node:24.18.0-bookworm` | Mermaid CLI runtime and Debian Chromium base |
| Mermaid CLI | 11.16.0 | Mermaid source to SVG smoke/backend candidate |
| Typst CLI | 0.15.0 | block math to SVG smoke/backend candidate |
| Chromium | Debian bookworm package | headless Mermaid rendering |
| Lua | 5.4 | WezTerm plugin syntax and API-shape smoke |
| ShellCheck | Debian bookworm package | product shell scripts |
| Noto fonts | Debian bookworm packages | Latin, CJK, emoji renderer coverage |

Node.js は LTS patch と OS family を固定します。再現性をさらに厳密にする
release candidate では、CI が解決した base image digest を `NODE_IMAGE` に記録します。

## 実行時依存

通常の `ptymark` バイナリは外部 runtime を要求しません。

- `PreviewRenderer` と `SourceRenderer`: Rust binary だけで動作
- Mermaid backend: 有効化する場合だけ `mmdc` と Chromium が必要
- Typst backend: 有効化する場合だけ `typst` が必要
- WezTerm plugin: WezTerm とホスト OS 用 `ptymark` binary が必要

外部 renderer は adapter を実装してから opt-in で有効化します。ツール不在、
timeout、非ゼロ終了、出力上限超過の場合は既定で元ソースへ fallback します。

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
- Node.js、Mermaid CLI、Typst、Lua、Chromium の起動
- global npm package の Mermaid CLI version
- Dockerfile、Compose fallback、env file の一致
- Rust format、Clippy、unit/integration tests
- WezTerm plugin smoke
- Mermaid SVG と Typst SVG の生成
- release archive の作成

## ライセンスと配布

- repository 本体: Apache-2.0
- Rust dependency: `Cargo.lock` と release 前の license inventory で確認
- npm dependency: Mermaid CLI と transitive package inventory を release evidence に保存
- apt dependency: base image/OS package manifest を release evidence に保存
- renderer が生成物へ font を埋め込む場合は font license と attribution を確認

外部ツールを binary archive に同梱しません。配布 archive は `ptymark` binary、
README、LICENSE、WezTerm plugin だけを含めます。
