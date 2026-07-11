<!--
@dependency-start
contract policy
responsibility Provides a Japanese repository audit checklist for this template.
upstream design ../AGENTS.md defines runtime and closeout gates.
upstream design ../vendor/agent-canon/documents/REVIEW_PROCESS.md defines review evidence and merge gates.
upstream design ../vendor/agent-canon/documents/agent-canon-github-remote.md defines AgentCanon remote policy.
upstream design template-github-remote.md defines Template remote policy.
upstream design ../vendor/agent-canon/documents/FILE_CHECKLIST_OPERATIONS.md defines routine operation checks.
@dependency-end
-->

# リポジトリ監査チェックリスト

このチェックリストは、template repo と派生 repo の状態を監査するときの確認項目です。
差分だけでなく、repo 全体の runtime surface、AgentCanon pin、検証コマンド、文書導線を確認します。

## この文書の読み方

- この checklist は、template / 派生 repo の監査観点と完了判定を 1 回の確認順にまとめます。
- 前半は Git、AgentCanon pin、MCP、runtime surface、dependency graph、文書導線を確認し、後半は workflow、tooling、Docker、実験 artifact、GitHub Actions、push 判定を確認します。
- repo 全体の状態監査、移行前後の健全性確認、closeout 前の抜け漏れ確認で使います。

## 監査メタ情報

- [ ] 監査日:
- [ ] 監査者:
- [ ] 対象 repo:
- [ ] 対象 branch:
- [ ] 対象 commit:
- [ ] 比較対象 remote:
- [ ] 監査結果: `pass` / `revise` / `blocked`
- [ ] block 理由:

## 1. Git と Remote

- [ ] `git status --short --branch --untracked-files=all` を確認した
- [ ] 作業開始時点の dirty file を user 変更、生成物、今回変更に分類した
- [ ] `origin` が GitHub canonical repo を向いている
- [ ] `main` が `origin/main` と意図通り一致している
- [ ] push 先が GitHub canonical であることを確認した
- [ ] commit message に remote migration や AgentCanon pin 変更の理由が残っている

確認コマンド:

```bash
git status --short --branch --untracked-files=all
git remote -v
git rev-parse HEAD
git rev-parse origin/main
```

## 2. AgentCanon Latest と Submodule

- [ ] `make agent-canon-ensure-latest` が pass している
- [ ] `vendor/agent-canon` の pin が AgentCanon GitHub `main` と一致している
- [ ] `.gitmodules` の `vendor/agent-canon.url` が GitHub canonical repo を向いている
- [ ] GitHub 操作の protocol が `gh auth status` と矛盾していない
- [ ] SSH 利用時は `github.com` の host key と GitHub auth が通る
- [ ] HTTPS 利用時は非対話 fetch が credential error で止まらない
- [ ] shared root surface drift がある場合は `bash tools/sync_agent_canon.sh link-root` で修復済み

確認コマンド:

```bash
make agent-canon-ensure-latest
git submodule status vendor/agent-canon
git config -f .gitmodules submodule.vendor/agent-canon.url
gh auth status
gh config get git_protocol -h github.com
git -C vendor/agent-canon rev-parse HEAD
git -C vendor/agent-canon ls-remote origin main
```

## 3. MCP と Codex Runtime

- [ ] `.codex/config.toml` に `repo_mcp_server` が定義されている
- [ ] canonical launcher が `bash mcp/repo_mcp_server.sh` になっている
- [ ] MCP inventory check が pass している
- [ ] MCP 起動失敗時に ad hoc local process へ置換していない
- [ ] `mcp/` が AgentCanon の対応 surface と一致している
- [ ] Hook や workflow から MCP preflight が機械的に呼ばれる
- [ ] MCP status と通常の `git status` の差異がある場合、原因を確認済み

確認コマンド:

```bash
python3 tools/agent_tools/check_mcp_inventory.py --require repo_mcp_server
bash mcp/repo_mcp_server.sh --help
git status --short --branch --untracked-files=all
```

## 4. Runtime Surface と Link 構成

- [ ] root `agents/` は `vendor/agent-canon/agents` の runtime view として整合している
- [ ] root `.agents/` は `vendor/agent-canon/.agents` と整合している
- [ ] root `tools/` は `vendor/agent-canon/tools` と整合している
- [ ] root `mcp/` は `vendor/agent-canon/mcp` と整合している
- [ ] `AGENTS.md` は thin entrypoint として保たれている
- [ ] shared surface の変更は `vendor/agent-canon/` 側を正本としている
- [ ] template 固有の説明は `documents/` に置かれ、Dockerfile に焼き込まれていない

確認コマンド:

```bash
bash tools/sync_agent_canon.sh check
find agents .agents tools mcp -maxdepth 1 -type l -ls
git diff -- .agents AGENTS.md agents mcp tools
```

## 5. Dependency Header と Graph

- [ ] すべての human-authored text file に `@dependency-start` / `@dependency-end` がある
- [ ] 旧形式の dependency header が残っていない
- [ ] header の `responsibility` が file の責務を説明している
- [ ] `upstream` と `downstream` が人間と agent の読み順に役立つ粒度になっている
- [ ] 自己参照がない
- [ ] 循環参照がない
- [ ] 孤立 manifest がない
- [ ] 差分限定ではなく全 repo の dependency review を実行している

確認コマンド:

```bash
bash tools/agent_tools/run_repo_dependency_review.sh --fail-missing
python3 tools/agent_tools/check_dependency_headers.py
bash tools/agent_tools/check_dependency_header_format.sh --require-header
bash tools/agent_tools/check_dependency_graph.sh --print-edges
```

## 6. 文書と README 導線

- [ ] `README.md` が現在の repo 構造、AgentCanon 構成、主要 command を説明している
- [ ] `documents/README.md` から重要な正本文書へ辿れる
- [ ] stale path、旧 helper 名、削除済み workflow への参照が残っていない
- [ ] 長文文書は単体で目的、前提、手順、検証が読める
- [ ] Markdown の見出し階層、list、code block、link が崩れていない
- [ ] 新しい監査・運用文書が正本と重複していない
- [ ] 文書変更に `make docs-check` の evidence がある

確認コマンド:

```bash
make docs-check
python3 tools/docs/check_markdown_lint.py --check documents/repository-audit-checklist.md
python3 tools/docs/check_markdown_math.py documents/repository-audit-checklist.md
rg -n "TODO|FIXME|old|legacy|subtree" README.md documents agents tools
```

## 7. Workflow、Skill、Eval、Goal

- [ ] `$agent-orchestration` が repo task の最初に呼ばれる構成になっている
- [ ] task workflow が requirements、research、plan、design、implementation、review、closeout に分離されている
- [ ] eval は skill、workflow、subagent prompt、config、memory の改善判断を対象にしている
- [ ] eval 結果が artifacts または memory に蓄積される導線がある
- [ ] `/goal` または `goal.md` 利用時に初期目標、default criteria、repo 固有 criteria が分離されている
- [ ] adaptive improvement loop が反復ごとに評価、逸脱検出、prompt 修正、再評価を残す
- [ ] subagent lifecycle が fresh task ごとに再起動され、closeout 前に close される
- [ ] 明示依頼なしの spawn を runtime 上位制約で強制しようとしていない

確認コマンド:

```bash
python3 tools/agent_tools/evaluate_skill_workflow_prompts.py --help
python3 tools/agent_tools/goal_loop.py --help
python3 tools/agent_tools/check_convention_compliance.py
rg -n "agent-orchestration|adaptive-improvement-loop|goal|eval|subagent_lifecycle" agents documents tools AGENTS.md
```

## 8. Tooling と静的解析

- [ ] `make agent-checks` が pass している
- [ ] `make ci` が pass している
- [ ] Python 変更では `pyright`、`ruff`、`pytest` が pass している
- [ ] C / C++ 変更では project-native configure、build、test が pass している
- [ ] hardcoded number、static `Any`、log helper naming、OOP readability の tool が必要範囲で pass している
- [ ] tool の重複実装や legacy 配置が OOP check 用など責務別に整理されている
- [ ] 新規 tool は既存 tool の option 追加や薄い adapter で済まない理由が記録されている
- [ ] vector search smoke で tool discovery が機能している

確認コマンド:

```bash
make agent-checks
make ci
python3 -m pyright
python3 -m ruff check python tests --select D,E,F,I,UP
python3 -m pytest tests/ -q --tb=short
python3 tools/agent_tools/vector_search.py --query "dependency review" --limit 5
```

## 9. Docker、Dev Container、Jupyter

- [ ] `gh` CLI は Docker image に焼かれず、shared `.devcontainer/post-create.sh` が workspace mount 後に導入している
- [ ] `docker/Dockerfile` に Codex CLI、GitHub CLI、Node/npm など agent convenience tooling が入っていない
- [ ] 初回 GitHub auth は user が実行する前提になっている
- [ ] 初回 Codex auth は host 側で `codex login` し、container / devcontainer は host `~/.codex` mount を再利用している
- [ ] devcontainer attach banner が `host-codex-home` と `codex-login` を表示している
- [ ] `~/.ssh` など host 側 SSH 設定の共有方針が devcontainer に反映されている
- [ ] Jupyter Notebook が container 内で起動できる
- [ ] `docker/register_safe_directories.sh /workspace` が `/workspace` と `vendor/*` 由来の `/workspace/vendor/<name>` を `safe.directory` に登録する
- [ ] `.devcontainer/devcontainer.json` の `postCreateCommand` が safe.directory 登録 helper を呼ぶ
- [ ] `docker/packs/default.toml` の smoke が vendor safe.directory 登録を検証している
- [ ] Dockerfile に Template / AgentCanon の machine-local remote path が焼き込まれていない
- [ ] `docker/requirements.txt`、`docker/README.md`、`.devcontainer/` が矛盾していない
- [ ] Docker dependency validator が pass している

確認コマンド:

```bash
bash tools/docker_dependency_validator.sh
python3 tools/ci/container_config.py
make docker-build-check
python3 tools/ci/run_container_pack.py --pack docker/packs/default.toml --print-only
```

## 10. 再利用、OOP、数理と実装境界

- [ ] 新規実装前に既存 helper、既存 tool、既存 workflow、既存 fixture を探索している
- [ ] `Reuse Survey` に見た path、再利用した path、不採用候補、不足理由が残っている
- [ ] OOP 的に不要な state、member、helper、wrapper、整形関数を増やしていない
- [ ] `None` runtime 判定で曖昧にせず、型で静的解析へ渡している
- [ ] `Any` が public boundary や新規 code path に増えていない
- [ ] 数理上の object、algorithm、implementation boundary が一致している
- [ ] hardcoded number が定数、設定、または根拠付き literal として整理されている
- [ ] 可読性評価は tool 出力と reviewer judgement を分けて扱っている

確認コマンド:

```bash
python3 tools/agent_tools/check_static_any.py --help
python3 tools/agent_tools/check_hardcoded_numbers.py --help
python3 tools/oop/python/readability.py --help
python3 tools/agent_tools/oop_rule_inventory.py --help
rg -n "Any|None|TODO|FIXME|_log|hardcoded" python tests tools
```

## 11. 結果ログ、可視化、Artifact

- [ ] run bundle は `reports/agents/<run-id>/` に保存されている
- [ ] `user_request_contract.md` に must-do、must-not-do、completion-evidence clause がある
- [ ] `schedule.md` が空でない
- [ ] `work_log.md` が作業開始から closeout まで更新されている
- [ ] `verification.txt` が `status=pass` になっている
- [ ] `closeout_gate.md` が user completion unlocked になっている
- [ ] eval、monitoring、feedback、改善判断の保存先が明示されている
- [ ] 可視化対象の結果ログが `reports/`、`notes/`、`memory/` のどこにあるか説明できる

確認コマンド:

```bash
find reports/agents -maxdepth 2 -type f | sort | tail -50
rg -n "status=pass|user_completion_report=unlocked|eval|feedback|monitoring" reports notes memory agents documents
```

## 12. 派生 Repo 監査

- [ ] 派生 repo の `vendor/agent-canon` pin が GitHub AgentCanon `main` と一致している
- [ ] 派生 repo 固有の AgentCanon 差分がある場合、dedicated GitHub branch と AgentCanon PR に分離されている
- [ ] Template 由来 repo では root surface が Template と構造的に一致している
- [ ] repo 固有の差分は `documents/`、project code、config に限定され、shared canon に混入していない
- [ ] `make agent-canon-ensure-latest` と `bash tools/sync_agent_canon.sh check` が派生 repo でも pass している

確認コマンド:

```bash
git remote -v
git submodule status vendor/agent-canon
```

## 13. GitHub Actions と PR Checklist

- [ ] `.github/workflows/ci.yml` が submodule-aware checkout を使う
- [ ] `.github/workflows/ci.yml` が最小権限 `permissions` と stale run 用 `concurrency` を持つ
- [ ] `.github/workflows/docker-build.yml` が submodule-aware checkout、最小権限、concurrency を持つ
- [ ] `.github/workflows/agent-coordination.yml` は AgentCanon 正本から root copy へ同期されている
- [ ] Agent coordination workflow の各 job が AgentCanon submodule を checkout する
- [ ] Template default PR checklist が repo-local 変更、AgentCanon pin、Docker、GitHub workflow、validation evidence を分けている
- [ ] Template 側 AgentCanon PR checklist が shared canon source、root surface sync、GitHub evidence を要求している
- [ ] Standalone AgentCanon repo 用の独立 PR checklist が `vendor/agent-canon/.github/PULL_REQUEST_TEMPLATE.md` にある
- [ ] GitHub automation と PR checklist が Codex workflow から辿れる
- [ ] PR checklist が未実行 command を pass と書かない運用になっている

確認コマンド:

```bash
python3 - <<'PY'
from pathlib import Path
import yaml
for path in sorted(Path('.github/workflows').glob('*.yml')):
    yaml.safe_load(path.read_text())
    print(f'{path}: yaml=pass')
PY
rg -n "submodules: false|checkout_agent_canon_submodule|permissions:|concurrency:|PULL_REQUEST_TEMPLATE|agent-canon-pr-workflow" .github vendor/agent-canon/.github agents documents
```

## 14. Push と完了判定

- [ ] 変更を commit 済み
- [ ] GitHub canonical remote へ push 済み
- [ ] `git status --short --branch --untracked-files=all` が clean
- [ ] `git log --oneline --decorate -5` で対象 commit が確認できる
- [ ] 未完了の planned work、review finding、validation、commit、push、follow-up 判断が残っていない
- [ ] user-facing completion report に未実行 check を pass と書いていない

確認コマンド:

```bash
git log --oneline --decorate -5
git status --short --branch --untracked-files=all
git push origin main
```

## 最終監査判定

- [ ] `pass`: 監査対象は現行規約、latest pin、検証、文書導線を満たしている
- [ ] `revise`: 修正すれば pass にできる項目がある
- [ ] `blocked`: auth、network、toolchain、未解決 conflict など監査を完了できない blocker がある

判定メモ:

```text
summary=
required_fixes=
validation_evidence=
push_evidence=
residual_risk=
```
