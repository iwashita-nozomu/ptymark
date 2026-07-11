# vendor
<!--
@dependency-start
contract design
responsibility Documents vendor for this repository.
upstream design agent-canon/documents/agent-canon-subtree-migration.md shared canon submodule update and legacy migration contract
downstream design agent-canon/README.md vendored shared canon overview
@dependency-end
-->

`vendor/` は、外部で管理される shared asset を、この repo から pinned dependency として参照する場所です。

この template では、shared agent canon の取り込み先を次に固定します。

- `vendor/agent-canon/`

原則:
- product runtime の正面入口は root `AGENTS.md` と root `.codex/` に残します
- `vendor/agent-canon/` は shared canon の Git submodule pin として扱います
- shared canon の通常更新は [tools/update_agent_canon.sh](../tools/update_agent_canon.sh) から行います
- root の shared symlink、GitHub path constraint copy、template-local regular file、root から消す standalone-only file は `vendor/agent-canon/documents/shared-runtime-surfaces.toml` を正本にします
- `documents/README.md`、template bootstrap、host/server/remote contract 系は template / derived repo の regular file です
- AgentCanon-owned policy docs such as `SHARED_RUNTIME_SURFACES.md` and `github-copilot-configuration.md` stay under `vendor/agent-canon/documents/` in template roots; they are not root `documents/` views
- `.github/workflows/agent-coordination.yml` と `.github/PULL_REQUEST_TEMPLATE/agent_canon.md` は、この submodule pin を正本とする root 同期コピーにします

よく使うコマンド:

```bash
bash tools/update_agent_canon.sh plan
bash tools/update_agent_canon.sh apply
bash tools/update_agent_canon.sh merge-main-into-current
bash tools/sync_agent_canon.sh status
bash tools/sync_agent_canon.sh link-root
bash tools/sync_agent_canon.sh check
```

Legacy subtree operations:
- `bash tools/sync_agent_canon.sh pull` と `bash tools/sync_agent_canon.sh push` は subtree-era compatibility または maintainer 低レベル操作です
- submodule 化済み repo の通常更新では `update_agent_canon.sh plan -> apply` を使います
- submodule 内の local commit は `merge-main-into-current` で GitHub `main` を取り込み、通常の AgentCanon GitHub branch / PR に回します

注意:
- `vendor/agent-canon/AGENTS.md` は、standalone AgentCanon repo 用 entrypoint として扱います
- product 全体の runtime discovery は root entrypoint に寄せます
- `check` は shared surface の drift を fail-fast で検出します
- `link-root` は shared surface の symlink と同期コピーを vendor 正本へ戻します。対象に未 commit の変更がある場合は、先に commit / stash するか、意図的な再同期だけ `AGENT_CANON_FORCE_RELINK=1` を使います
