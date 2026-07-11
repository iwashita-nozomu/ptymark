# scripts
<!--
@dependency-start
contract design
responsibility Documents scripts for this repository.
upstream design ../documents/template-bootstrap.md bootstrap contract
downstream implementation start_repository.sh repository start wrapper
@dependency-end
-->

`scripts/` は repo-local bootstrap の置き場です。
shared automation は `tools/` を使います。

## ここに置くもの

- clone 直後の初期化
- project slug や display name の置換
- bare remote 名の初期化
- GitHub-backed `agent-canon` submodule 前提の初期化

## 置かないもの

- agent helper
- CI / review / validation
- Docker / container runner
- experiment helper
- Markdown 整備

それらは `tools/` に置きます。

## 現在の入口

- [init_from_template.sh](init_from_template.sh)
  - clone 直後に project slug、display name、project remote 名などを初期化します。
- [start_repository.sh](start_repository.sh)
  - `$start-repository` skill から呼ぶ token-efficient wrapper です。
  - 既定では dry-run、clean clone なら init 前の `make agent-canon-ensure-latest`、初期化、必要な validation までを 1 command にまとめます。dirty state なら preflight の未実行理由を出します。`--force` を init に渡す場合は wrapper preflight を block 扱いで skip します。
  - init 変更を commit したあとは `--validate-only` で `make fresh-clone-check` と `make ci-quick` まで流します。
  - AgentCanon は GitHub submodule を正本とし、project-local `agent-canon` bare repo は作りません。

## 参照先

- [tools/README.md](../tools/README.md)
- [documents/tools/README.md](../vendor/agent-canon/documents/tools/README.md)
- [documents/SHARED_RUNTIME_SURFACES.md](../vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md)
