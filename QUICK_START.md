# Quick Start
<!--
@dependency-start
contract design
responsibility Documents Quick Start for this repository.
upstream design README.md primary repository overview
upstream design AGENTS.md runtime agent entrypoint
upstream design vendor/agent-canon/CONTAINER_OPERATIONS.md AgentCanon container and devcontainer operation rulebook
@dependency-end
-->

このファイルは、作業を開始するための最短入口です。テンプレート全体像は
[README.md](README.md) を先に読むと流れが付きます。

## この文書の読み方

- この quick start は、repo を触り始めるための最短読了順、開始コマンド、実装前確認、終了時整理を扱います。
- `最初に読む` で profile 別の入口を選び、作業開始、実装前確認、よく使うコマンド、実験、環境、終了整理を順に確認します。
- clone 直後、短い作業開始、必要な validation route を素早く決めるときに読みます。

## 1. 最初に読む

- [documents/README.md](documents/README.md)
- [README.md](README.md): repository-wide map
- [Runtime Profiles And Check Matrix](vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md)

必要な profile だけ追加で読みます。

- bootstrap: [documents/template-bootstrap.md](documents/template-bootstrap.md)
- agent runtime: [agents/README.md](agents/README.md), [agents/workflows/README.md](agents/workflows/README.md)
- Python: [coding-conventions-python.md](vendor/agent-canon/documents/coding-conventions-python.md)
- C++: [cpp-build-layout.md](vendor/agent-canon/documents/cpp-build-layout.md)
- host/runtime: [linux-wsl-host-requirements.md](documents/linux-wsl-host-requirements.md)

実験を扱う場合は追加で次を見ます。

- [agents/workflows/experiment-workflow.md](agents/workflows/experiment-workflow.md)
- [agents/workflows/research-workflow.md](agents/workflows/research-workflow.md)
- [documents/experiment-registry.md](vendor/agent-canon/documents/experiment-registry.md)
- [experiments/README.md](experiments/README.md)

agent を使う場合は次を見ます。

- [agents/README.md](agents/README.md)
- [documents/AGENTS_COORDINATION.md](vendor/agent-canon/documents/AGENTS_COORDINATION.md)
- [documents/agent-canon-parent-repo-latest-checklist.md](vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md)

## 2. 作業の始め方

- 既定の統合先は `main` です。
- 短期 branch は必要なときだけ切り、長期の分岐運用は避けます。
- branch 側で file 構成を変えた場合は、`agents/workflows/main-integration-workflow.md` を見て integration worktree で戻します。
- 変更の前に、対象ディレクトリと必要な更新を先に決めます。
- Python と Markdown は base profile ですが、validation は changed path と risk class に合わせます。

template から起こした直後に repo 名や bare remote 名を変えるなら wrapper から始めます。

```bash
bash scripts/start_repository.sh --project-slug your-project --display-name "Your Project"
```

最低限の確認:

```bash
git status --short
make check-matrix
```

Python code を触る場合は targeted pytest / pyright / ruff を足します。
shared canon を触る場合は `make agent-canon-pr-check`、Docker を触る場合は
`make docker-check` と必要なら `make docker-build-check` を使います。
AgentCanon が submodule の repo では、親 repo に無関係な dirty path があっても AgentCanon update surface が clean なら `make agent-canon-ensure-latest` を先に実行できます。

## 3. 実装前の確認

- `vendor/agent-canon/documents/conventions/README.md` と `vendor/agent-canon/documents/coding-conventions-python.md` を先に見ます。
- agent workflow を使う変更なら `agents/workflows/README.md` と `agents/canonical/CODEX_WORKFLOW.md` を確認します。
- AgentCanon submodule / root view を触る変更なら `vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md` と `vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md` で owner class を確認します。

```bash
make docs-check
python3 -m pytest tests/ -q --tb=short   # Python code/tests changed
python3 -m pyright                       # Python code/tests changed
python3 -m ruff check python tests --select D,E,F,I,UP  # Python code/tests changed
```

フルチェックが必要な変更:

```bash
make ci
```

`make ci` は full confidence gate です。docs-only、profile-local、または narrow
code edit では check matrix に従った個別 check を evidence にできます。

## 4. よく使うコマンド

```bash
make check-matrix
make ci-quick
make ci
make docs-check
make clean-generated
make experiment-check
make docker-build-check
bash tools/run_comprehensive_review.sh
```

## 5. 実験の基本

- 実験コードは `experiments/<topic>/` に置きます。
- topic の正本 entrypoint と formal command は `experiments/registry.toml` に置きます。
- 新しい topic は `python3 tools/experiments/create_experiment_topic.py <topic>` で作ります。
- 実行ごとの生成物は `experiments/<topic>/result/<run_name>/` に置きます。
- 1 回の実験 report は `experiments/report/<run_name>.md` に置きます。
- server で formal run を回すときは `tools/experiments/run_managed_experiment.py --topic <topic> --use-registered-command formal` で `run_manifest.json` と `run.log` を残します。
- partial run を正式結果として扱いません。
- agent に実験つき改造 loop を回させる場合は `$adaptive-improvement-loop` と `$experiment-lifecycle` を使います。

## 6. 環境の基本

- Docker は supported runtime profile の一つです。host/lightweight path と Docker path は同格に扱います。
- host 前提は `documents/linux-wsl-host-requirements.md` を正本にします。
- container runtime の再利用 surface は `docker/packs/*.toml` と `tools/ci/run_container_pack.py` を基準にします。
- container / devcontainer の責務境界は `vendor/agent-canon/CONTAINER_OPERATIONS.md` を正本にします。
- Python 依存を追加する場合は `docker/requirements.txt` を更新し、導入は post-create / runtime setup の `docker/install_python_dependencies.sh` に集約します。
- `docker/Dockerfile`、`docker/requirements.txt`、post-create、または runtime setup を更新したら `bash tools/docker_dependency_validator.sh` を先に流し、image / pack smoke に影響する変更では `make docker-build-check` も流します。
- repo-wide な tool 導入案や Docker 変更では `agents/templates/environment_change_proposal.md` に triggering code requirement と blocked command を先に記録します。
- container 内では `PYTHONPATH=/workspace/python` を前提にします。
- Jupyter notebook runtime は workspace mount 後の `docker/install_python_dependencies.sh` で導入します。
- host browser から container 内 JupyterLab を使う場合は `make docker-jupyter` を実行し、`http://127.0.0.1:8888/lab?token=project-template` を開きます。port / token は `JUPYTER_HOST_PORT` と `JUPYTER_TOKEN` で変更できます。
- repo-local `.venv` は host では作らず、container 内だけ `make python-env-status` と `make python-env-prepare` を使います。
- C++ を使う場合の canonical CMake entrypoint は root `CMakeLists.txt` です。
- out-of-source build tree は `build/cpp/<profile>/`、再利用する local install tree は `.state/cpp-install/<profile>/` に置きます。
- Markdown の体裁ルールは `.markdownlint.json` と `vendor/agent-canon/documents/conventions/common/05_docs.md` を基準にします。
- `pipdeptree` と `deptry` は dependency profile の tool です。dependency 変更時または scheduled audit で使います。

`docker/Dockerfile` は project runtime、shared `.devcontainer/` は Codex / GitHub CLI /
host mount などの agent ergonomics を持ちます。詳細な境界、例外、validation は
`vendor/agent-canon/CONTAINER_OPERATIONS.md`、template 固有の実行手順は
`docker/README.md` を見ます。

```bash
docker build -t project-template -f docker/Dockerfile .
docker run --rm -it \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v $(pwd):/workspace -w /workspace \
  project-template bash
bash .devcontainer/post-create.sh /workspace
codex --version
gh --version
docker --version
make python-env-status
make python-env-prepare
```

該当 profile の build 可否だけを確認したい場合は次です。

```bash
make docker-build-check
make docker-build-check-host-docker
make server-check
cmake -S . -B build/cpp/dev
cmake --build build/cpp/dev
ctest --test-dir build/cpp/dev --output-on-failure
python3 tools/ci/run_container_pack.py --pack docker/packs/default.toml --print-only
python3 tools/ci/run_codex_in_repo_container.py --print-only
```

## 7. 終了時の整理

```bash
git status --short
git branch --show-current
bash tools/push_origin.sh
```

- 長期に残す知見は `notes/` に寄せます。
- worktree を使った場合は、`vendor/agent-canon/documents/notes-lifecycle.md` を見て action log から knowledge/theme/failure へ昇格させる項目を決めます。
- repo-local な active contract は `documents/`、shared AgentCanon rule は `vendor/agent-canon/documents/` に反映します。
- push / PR / branch 削除は repo policy と user authorization に従います。短期 branch を使った場合は、統合後に削除して `main` に戻します。
