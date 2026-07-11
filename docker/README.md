# Docker Runtime
<!--
@dependency-start
contract environment
responsibility Documents Docker Runtime for this repository.
upstream design ../documents/linux-wsl-host-requirements.md host runtime requirements
upstream design ../vendor/agent-canon/CONTAINER_OPERATIONS.md AgentCanon container and devcontainer operation rulebook
downstream environment packs/default.toml default container pack
downstream environment packs/default-host-docker.toml host-docker container pack
@dependency-end
-->

`docker/` は、この template の共通開発環境と container runtime 運用の正本です。
単に `docker build` を置くだけでなく、build / smoke / workspace mount / nested Codex を pack と profile で再利用できる形にします。

host 側の前提は [linux-wsl-host-requirements.md](../documents/linux-wsl-host-requirements.md) を正本にします。

Container / devcontainer の責務境界は
[CONTAINER_OPERATIONS.md](../vendor/agent-canon/CONTAINER_OPERATIONS.md) が正本です。
この file は template repo の `docker/` 実装 runbook であり、AgentCanon policy を再定義しません。
矛盾した場合は `CONTAINER_OPERATIONS.md` を直し、その後でこの runbook と validator を追従させます。

## この文書の読み方

- この runbook は、template repo の `docker/` 実装、runtime pack、container 実行入口を扱います。
- `Primary Files` と `Runtime Pack` で正本 file と pack 構成を確認し、nested Codex、Python、Jupyter、Docker-in-Docker、dev container は該当 section を読みます。
- Docker profile、pack smoke、container 内実行、runtime 更新の入口を確認するときに読みます。

## Primary Files

- `Dockerfile`
  - canonical container image 定義です。OS package、project runtime / build tool、Docker CLI までを入れ、Codex CLI、Codex 用 Node/npm、GitHub CLI は入れません。
- `requirements.txt`
  - repo-wide の Python 依存リストです。Jupyter、JSONL、Graphviz 周辺の結果可視化依存もここで管理します。
- `install_python_dependencies.sh`
  - workspace mount 後に `requirements.txt` を導入する唯一の script です。devcontainer post-create と pack smoke が同じ入口を使います。
- `packs/default.toml`
  - 既定 build / smoke pack です。
- `packs/default-host-docker.toml`
  - host Docker socket を mount して daemon 到達性も見る pack です。
- `codex-container-profiles.toml`
  - nested Codex 実行 profile の正本です。
- `python-execution-rules.toml`
  - `run_python_in_dockerfile.py` が Python file をどの pack で動かすか決める規則です。

## Runtime Pack

build と smoke は `Dockerfile` を直接たたく代わりに、runtime pack を使います。
runtime pack は `docker/packs/*.toml` に置き、container runtime の唯一の正本にします。devcontainer も repo-defined container runner も同じ pack と shared helper から派生させます。
runtime pack には次を 1 つの spec としてまとめます。

- Dockerfile path
- build context
- optional build target
- 一時 image tag
- smoke command 群
- runtime env / mounts / workdir

既定 pack:

- `docker/packs/default.toml`
- `docker/packs/default-host-docker.toml`

主な入口:

- `python3 tools/ci/run_container_pack.py`
  - pack 定義から build と smoke を実行します。
- `bash docker/check_build.sh`
  - GitHub Docker Build workflow と `make docker-build-check` が使う submodule-aware build gate です。root `.dockerignore` は image build context から `vendor/agent-canon` を除外しますが、runtime smoke は checkout 済み `vendor/agent-canon` から shared `.devcontainer/post-create.sh` を使います。
- `python3 tools/ci/run_in_repo_container.py`
  - pack 定義から repo workspace を mount した container command を実行します。
- `python3 tools/ci/run_repo_program.py`
  - Python file、shell script、workspace binary、plain command を 1 つの入口で container 実行し、先に environment check も流します。
- `python3 tools/ci/run_python_in_dockerfile.py`
  - Python file path と rule に基づいて適切な pack で実行します。
- `python3 tools/ci/run_codex_in_repo_container.py`
  - repo を mount した canonical container 内で nested Codex を起動します。
- `bash .devcontainer/generate-runtime-compose.sh`
  - devcontainer 用の compose を canonical pack から root-local に生成します。template / derived repo では `.devcontainer/` が AgentCanon-owned root view なので、実行前に AgentCanon submodule checkout が必要です。

## Nested Codex

`run_codex_in_repo_container.py` は、repo の canonical runtime を build し、その中で `codex` を起動する入口です。
実行 profile の正本は `docker/codex-container-profiles.toml` です。
project-scoped Codex config の正本は `.codex/config.toml` で、template 既定では `approval_policy = "never"` と `sandbox_mode = "danger-full-access"` を使います。つまり container 内で起動した Codex も、`jax_solver_util` と同じく最初から full access 前提です。
Codex 認証は host-local state を正本にします。host 側で `codex login` または API key login を済ませ、container は host `~/.codex` の mount または `OPENAI_API_KEY` forward を使います。container 内で別の永続 auth state を作る運用は避けます。
Codex CLI、Codex 用 Node/npm、GitHub CLI は Docker image へ焼かず、workspace mount 後に shared `.devcontainer/post-create.sh` で導入します。nested Codex runner は setup だけ root で行い、Codex 起動前に host `uid:gid` へ落とします。

既定の挙動は次です。

- `default` profile を使う
- setup 後に host の `uid:gid` で Codex を実行する
- `HOME=/workspace/.state/nested-codex/<profile>` を使う
- host の `~/.codex` を `"$HOME/.codex"` として直接 mount する
- repo の `.codex/config.toml` から full-access default を読む
- host の `~/.gitconfig` と `~/.git-credentials` があれば持ち込む
- `SSH_AUTH_SOCK` があれば forward する

よく使う例:

```bash
python3 tools/ci/run_codex_in_repo_container.py --list-profiles
python3 tools/ci/run_codex_in_repo_container.py --print-only
python3 tools/ci/run_codex_in_repo_container.py
python3 tools/ci/run_codex_in_repo_container.py --profile host-docker
python3 tools/ci/run_codex_in_repo_container.py --share-host-codex-home
python3 tools/ci/run_codex_in_repo_container.py --no-seed-host-codex --forward-env OPENAI_API_KEY
```

## Repo Program Runner

`run_repo_program.py` は、repo 内 program を container で回す入口を 1 つにまとめた wrapper です。
次を同じ書式で扱います。

- Python file
- shell script
- workspace binary
- plain command

既定では target 実行前に軽量 environment check も実行します。
通常の target 実行と environment check は、workspace mount 後に `docker/install_python_dependencies.sh` を先に実行します。

よく使う例:

```bash
python3 tools/ci/run_repo_program.py docker/check_build.sh -- --pack docker/packs/default.toml
python3 tools/ci/run_repo_program.py python3 -- --version
python3 tools/ci/run_repo_program.py --skip-env-check --print-only cmake -- --version
```

## Python File Runner

`run_python_in_dockerfile.py` は、Python file を container で再現したいときの入口です。
規則の正本は `docker/python-execution-rules.toml` です。

この file では、少なくとも次を rule として持ちます。

- `dockerfile`
- `match_roots`
- `pack`
- `python_bin`
- `workdir`

よく使う例:

```bash
bash tools/docker_dependency_validator.sh
python3 tools/ci/run_python_in_dockerfile.py docker/Dockerfile tools/docs/check_markdown_math.py -- README.md
```

## Python Environment Rule

canonical environment は Docker image、`docker/requirements.txt`、および workspace mount 後に走る `docker/install_python_dependencies.sh` です。
repo-local `.venv` は host runtime では作らず、container runtime でだけ canonical tool から許可します。

Python module install は Docker image build では実行しません。Template の Docker build は
`vendor/agent-canon` submodule や workspace Python package に依存しない OS / project runtime layer
までを作り、Python module は `.devcontainer/post-create.sh` から
`docker/install_python_dependencies.sh` を呼んで導入します。default runtime pack の smoke も
同じ post-create entrypoint を先に実行するため、devcontainer と pack smoke の依存導入経路は
1 本だけです。

許可事項:

- container 内で `python3 tools/ci/python_env_policy.py --create` を使って `.venv` を作る
- notebook kernel や補助 package を `.venv` に追加したい場合も `.venv` だけを使う

禁止事項:

- host runtime で repo-local `.venv` を作る
- `venv/`、`env/`、`.conda/`、`conda-env/`、`.venv-*` を workspace に作る
- canonical tool を通さずに別経路の env manager を repo の既定手順にする

環境 drift check は Python に依存しない次の入口を使います。

```bash
bash tools/docker_dependency_validator.sh
python3 tools/ci/python_env_policy.py
python3 tools/ci/python_env_policy.py --create
```

`.venv` 作成時は `--system-site-packages` を使い、post-create / runner setup 後に container runtime へ入った Jupyter / analysis / test package をそのまま見せます。

## Jupyter Notebook

notebook runtime package は `docker/requirements.txt` を正本にし、
`.devcontainer/post-create.sh` が container 作成後に導入します。VS Code では `ms-toolsai.jupyter` を推奨拡張として配布済みです。

host browser から container 内 JupyterLab を開く既定入口:

```bash
make docker-jupyter
```

VS Code では `Tasks: Run Task` から `Docker: Start JupyterLab` を選ぶと同じ入口を実行します。

既定では container port `8888` を host `8888` に公開し、token は `project-template` です。
port や token を変える場合:

```bash
JUPYTER_HOST_PORT=8890 JUPYTER_TOKEN=my-token make docker-jupyter
```

起動後は `http://127.0.0.1:8888/lab?token=project-template` を開きます。
`JUPYTER_HOST_PORT` を変えた場合は URL の port も同じ値にします。

よく使う流れ:

```bash
make python-env-status
make python-env-prepare
. .venv/bin/activate
python -m ipykernel install --user --name project-template
jupyter lab --ip=0.0.0.0 --no-browser
```

VS Code で notebook を開く場合は、container 内の `.venv/bin/python` を kernel として選べば十分です。host 側に `.venv` を作る運用は canonical にしません。

## Result Logs And Visualization

結果ログの保持、summary、可視化 artifact の正本ルールは
[result-log-retention-and-visualization.md](../vendor/agent-canon/documents/result-log-retention-and-visualization.md)
です。canonical runtime では次を標準で使えるようにします。

- `graphviz` / `dot` for dependency and structural graph rendering
- JupyterLab / notebook / ipykernel for interactive result inspection
- `pydeps` and `snakeviz` for dependency and profiling visualization
- `tools/data/jsonl_to_md.py` for JSONL-to-Markdown summaries
- `tools/hlo/summarize_hlo_jsonl.py` for HLO summary JSON

既定 pack の smoke は `dot -V` と Python runtime import を確認します。result helper 自体は workspace mount 後の normal CI / docs check 側で確認し、Docker image build の入力にはしません。
Dockerfile、requirements、Python installer、runtime pack のいずれかを変えたら、まず境界検査を通します。Docker image または pack smoke に影響する変更では build check も通します。

```bash
bash tools/docker_dependency_validator.sh
bash docker/check_build.sh --pack docker/packs/default.toml
```

## Docker In Docker

この template が同梱するのは `docker` CLI だけです。
container 内から `docker build` / `docker run` を行う場合は、host の Docker socket を mount するか、別 daemon を使います。

host socket 前提の pack は `docker/packs/default-host-docker.toml` です。

```bash
python3 tools/ci/run_container_pack.py --pack docker/packs/default-host-docker.toml
python3 tools/ci/run_codex_in_repo_container.py --profile host-docker
```

`safe.directory` は `docker/Dockerfile` から `docker/register_safe_directories.sh` を呼んで `git config --global` に登録します。build 時は `/workspace` だけを登録し、devcontainer 作成時や smoke test では mount 済み workspace の `vendor/*` を列挙して `/workspace/vendor/<name>` を動的に登録します。Template / AgentCanon 固有の GitHub remote や local mirror 名は Dockerfile に焼かず、[Template GitHub Remote](../documents/template-github-remote.md) と [AgentCanon GitHub Remote](../vendor/agent-canon/documents/agent-canon-github-remote.md) を正本にします。

Template の Docker build context は root `.dockerignore` で `vendor/agent-canon` を除外します。shared canon を読む必要がある validation は workspace mount 後に実行し、image build の入力にはしません。

repo-defined container runner でも、host `~/.codex` が存在するときは `/root/.codex` へ自動 mount します。対象は少なくとも次です。

- `python3 tools/ci/run_in_repo_container.py`
- `python3 tools/ci/run_repo_program.py`
- `python3 tools/ci/run_container_pack.py`
- `python3 tools/ci/run_python_in_dockerfile.py`

つまり、dev container に入らず `make docker-run ARGS='...'` や `python3 tools/ci/run_repo_program.py ...` を使う場合でも、container 内の `~/.codex` は host state をそのまま使います。

## VS Code Dev Container

`.devcontainer/devcontainer.json` は 1 枚の generated Docker Compose file を使います。起動前に `.devcontainer/generate-runtime-compose.sh` を走らせます。template / derived repo ではこの script 自体が AgentCanon-owned root view なので、AgentCanon submodule checkout 後に実行します。script は repo-local `docker/packs/default.toml` を読み、host を見て次を自動切替します。

生成 compose には repo path 由来の unique project name を入れます。共有
`.devcontainer/devcontainer.json` の display name は
`${localWorkspaceFolderBasename}-devcontainer` とし、Docker Compose project
name は `<repo-slug>-<path-hash>-devcontainer` 形式にします。同名 clone などで
さらに明示したい場合だけ、host 側で `DEVCONTAINER_PROJECT_NAME` を指定します。
subnet / gateway / IPAM は固定せず、Docker Compose の default network 自動割当に任せます。

- NVIDIA GPU が見えるとき:
  - `gpus: all` を追加
- GPU が見えないとき:
  - CPU-only のまま起動
- `/mnt/git` が存在するとき:
  - `/mnt/git:/mnt/git` を bind mount
  - local bare remote への push/pull を container 内から継続できる
- `/mnt/git` が無いとき:
  - mount しない
  - dev container 自体は CPU/GPU 判定だけでそのまま起動する
- host `~/.codex` が存在するとき:
  - `${HOME}/.codex:/root/.codex` を bind mount
  - dev container 内の Codex auth / config は host と同じ state を使う
  - attach banner の `codex-login` で `codex login status` の結果を確認する
- host `~/.codex` が無いとき:
  - mount しない
- host `~/.config/gh` が存在するとき:
  - `${HOME}/.config/gh:/root/.config/gh` を bind mount
  - 初回 `gh auth login` は host 側で人間が行い、dev container はその認証 state を使う
- host `~/.ssh` が存在するとき:
  - `${HOME}/.ssh:/root/.ssh:ro` を read-only bind mount
  - private key と `known_hosts` は host 側を正本にし、repo には書かない
- host `SSH_AUTH_SOCK` が有効な socket を指すとき:
  - socket を `/ssh-agent` として bind mount
  - container 内では `SSH_AUTH_SOCK=/ssh-agent` を使う

そのため、template を clone したディレクトリでも、GPU なし環境で dev container が落ちにくくなります。

VS Code で attach した直後には `.devcontainer/post-attach.sh` を実行し、少なくとも次を banner で表示します。

- GPU の有無
- `/mnt/git` mount の有無
- host `~/.codex` mount の有無
- `codex login status` の結果
- host `~/.config/gh` mount の有無
- host `~/.ssh` mount の有無
- `SSH_AUTH_SOCK` forward の有無
- `gh auth status` の結果
- Docker socket mount の有無
- Codex の `approval_policy` と `sandbox_mode`
- `PYTHONPATH`

## GitHub CLI / SSH Sharing

GitHub CLI は canonical Docker image に同梱せず、shared `.devcontainer/post-create.sh` が workspace mount 後に導入します。初回認証は host で人間が行います。

```bash
gh auth login
gh auth status
```

dev container では `~/.config/gh` を mount するため、container 内の `gh` は host の認証 state を再利用します。SSH は `~/.ssh` を read-only mount し、可能な場合は `SSH_AUTH_SOCK` を forward します。

注意:

- `~/.config/gh` と `~/.ssh` は secret surface です。trusted workspace でだけ dev container を開きます。
- SSH host key や key file の追加・更新は host 側で行います。
- `git@github.com:...` の初回接続で host key が未登録の場合は、host 側で `ssh -T git@github.com` などを実行してから dev container を起動します。

VS Code の前提拡張は `.vscode/extensions.json` を正本にします。

- `ms-python.python`
- `ms-toolsai.jupyter`
- `ms-azuretools.vscode-docker`
- `ms-vscode-remote.remote-containers`
- `ms-vscode.cpptools`
- `ms-vscode.cmake-tools`

runtime 側の C/C++ 基本 tool は、すでに `docker/Dockerfile` に入っています。

- `build-essential`
- `pkg-config`
- `cmake`
- `ninja-build`

repo maintenance helper が前提にする host-compatible tool も canonical image に同梱します。

- `rsync`
  - `tools/ci/check_fresh_clone.sh` の workspace overlay で使います。
  - host に `rsync` が無い場合でも script は `tar` fallback で動きますが、canonical container では Dockerfile 側で `rsync` を提供します。

C++ を使う派生 repo に備えて、canonical image には次を同梱します。

- `python3.11-dev`
- `cmake`
- `ninja-build`
- `build-essential`

template 既定では `CMAKE_GENERATOR=Ninja` を image 側で固定します。
JAX export、IREE、XLA FFI header などの重い runtime は template default には含めません。
必要な project だけ `docker/requirements.txt`、`docker/Dockerfile`、project-local CMake module、smoke target を同じ変更で足します。

canonical CMake layout と build artifact の再利用方針は [cpp-build-layout.md](../vendor/agent-canon/documents/cpp-build-layout.md) を見ます。要点は次です。

- root `CMakeLists.txt`
  - canonical entrypoint
- `cmake/`
  - helper module
- `build/cpp/<profile>/`
  - out-of-source build tree
- `.state/cpp-install/<profile>/`
  - reusable local install tree
host に `docker` group が設定されていても、現在の shell がその group をまだ持っていない場合があります。`getent group docker` にユーザー名が出ても `id` に `docker` が無いときは、新しい login shell を開いてから `make docker-build-check` を実行します。一時確認だけなら `sg docker -c 'docker version'` で daemon 到達性を切り分けられます。

Linux / WSL2 host の前提、`/mnt/git` の扱い、workspace の置き場所は `documents/linux-wsl-host-requirements.md` を見ます。

## Standard Commands

```bash
make docker-build-check
make docker-build-check-host-docker
make server-check
make docker-shell
make docker-codex
make docker-codex-host-docker
cmake -S . -B build/cpp/dev
cmake --build build/cpp/dev
ctest --test-dir build/cpp/dev --output-on-failure
python3 tools/ci/run_container_pack.py --pack docker/packs/default.toml --print-only
python3 tools/ci/run_repo_program.py --print-only python3 -- --version
python3 tools/ci/run_in_repo_container.py --pack docker/packs/default.toml --shell-session --tty
```

## Update Rule

`docker/Dockerfile` や `docker/requirements.txt` を更新した変更では、次も同じ変更で見直します。

- `README.md`
- `QUICK_START.md`
- `vendor/agent-canon/documents/coding-conventions-project.md`
- この `docker/README.md`

必要なら `agents/templates/environment_change_proposal.md` に triggering code requirement、blocked command、影響範囲、validation、rollback を残します。
