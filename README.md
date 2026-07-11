# Project Template
<!--
@dependency-start
contract design
responsibility Documents Project Template for this repository.
upstream design AGENTS.md agent runtime entrypoint
upstream design LICENSE repository license text
upstream design vendor/agent-canon/CONTAINER_OPERATIONS.md AgentCanon container and devcontainer operation rulebook
downstream design QUICK_START.md quick-start reader path
downstream design documents/licensing-policy.md repository license boundary
@dependency-end
-->

> [!IMPORTANT]
> MCP server は起動成功率が低めです。MCP 前提の作業では、起動している前提で進めず、最初に接続状態と利用可否を確認してください。

> [!IMPORTANT]
> subagent と skill の起動を甘くしないでください。task が subagent / skill を要求する場合は、parent の手作業や暗黙 fallback で代替せず、必要 surface を明示して機械的に起動してください。未起動なら、その事実を最初に確認してから進めます。

実装、文書、必要に応じた実験・エージェント運用を 1 つの repo で扱うためのテンプレートです。
base profile は Python 実装と Markdown 文書を想定しますが、Docker、C++、実験、GitHub automation、memory は opt-in profile です。

この README は人間向けの入口です。Codex runtime が最初に読む repo instruction
surface は `AGENTS.md` で、`agents/README.md` は人間と agent が workflow /
skill / runtime hub を探すための次の入口です。

## この文書の読み方

- この README は、template repo の構造、基本方針、clone 後の進め方、日常コマンドの入口を扱います。
- `テンプレート構造` と `基本方針` で repo 全体の責務を確認し、clone、初期手順、実験、Docker、詳細入口は目的別 section を読みます。
- 新規 clone、派生 repo の立ち上げ、どの正本文書へ進むかを決めるときに最初に読みます。

## テンプレート構造

この repo は、project 固有の実装、実験、文書、開発環境、agent runtime を同じ root から扱えるように分けています。
clone 直後にまず見る入口はこの README、Codex runtime instruction の入口は `AGENTS.md`、agent workflow / skill の hub は `agents/README.md`、実際の初期化入口は `scripts/start_repository.sh` です。

```text
.
├── README.md                         # 人間向けの全体入口
├── QUICK_START.md                    # 最短の手動起動手順
├── AGENTS.md                         # agent runtime entrypoint。AgentCanon submodule pin への symlink
├── Makefile                          # 日常 check / bootstrap / validation の短い入口
├── pyproject.toml                    # Python project metadata と tool 設定
├── CMakeLists.txt                    # C++ profile を使う場合の root entrypoint
├── python/                           # Python 実装本体
├── tests/                            # pytest と runtime/tooling のテスト
├── documents/                        # repo-local index, active contracts, and project docs
├── notes/                            # durable knowledge profile のテーマ別メモ
├── references/                       # research profile の外部仕様や補助資料
├── agents/                           # agent runtime profile の root view。vendor への symlink
├── .agents/, .codex/                 # Codex / shared agent runtime view
├── vendor/agent-canon/               # shared agent canon の Git submodule pin
├── tools/                            # shared automation view。vendor への symlink
├── scripts/                          # repo-local bootstrap 専用 script
├── docker/                           # Docker runtime profile の元設定
├── .devcontainer/                    # devcontainer profile の entrypoint
├── .github/                          # GitHub automation profile の workflow と PR template
├── experiments/                      # experiment profile の topic、artifact、report
├── cmake/                            # C++ profile の helper module
├── src/, include/, lib/              # C / C++ profile の実装置き場
├── reports/                          # ignored runtime artifact / agent run bundle の生成先
└── .vscode/                          # editor profile の補助設定
```

### Runtime Profiles

この template は surface を最初から持ちますが、全 surface が常時必須ではありません。
profile と validation の正本は
[Runtime Profiles And Check Matrix](vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md)
です。

- Base project: README、QUICK_START、documents index、project code/tests。
- Agent runtime: `AGENTS.md`、`agents/`、`.agents/`、`.codex/`、`mcp/`、shared `tools/`。
- Environment: `docker/`、`.devcontainer/`、runtime packs、Jupyter。
- GitHub automation: `.github/`、Actions、PR templates。
- Experiment / research: `experiments/`、`references/`、managed run artifacts。
- C++: `CMakeLists.txt`、`cmake/`、`src/`、`include/`、`lib/`。
- Memory / notes: `memory/`、`notes/`、learning or durable feedback capture。

### Repo-Local と Shared Canon の境界

- `documents/`
  - repo-local index、template-owned active contract、project-owned design doc を置きます。
  - `documents/README.md` は repo-local 目次です。shared workflow / coding / review policy は `vendor/agent-canon/documents/` の AgentCanon 正本から読み、bootstrap / host / server contract は template または derived repo の regular file として扱います。
- `notes/`
  - 実験や調査をまたいで残したい知見、補助メモ、テーマ整理を置きます。
  - その場限りの run log ではなく、後続作業で再利用する知識だけを残します。memory / learning profile が active な時だけ closeout 対象にします。
- `agents/`
  - エージェントチーム定義、運用ルール、workflow canon の正本です。
  - root の `agents/` は `vendor/agent-canon/agents` への symlink です。shared workflow を直すときは `vendor/agent-canon/` 側を正本として扱います。
- `tools/`
  - shared automation、agent helper、CI/check、container runner の入口です。
  - agent helper、CI / review / validation、container runner、experiment helper、Markdown helper の実装はここに置きます。
  - root の `tools/` は `vendor/agent-canon/tools` への symlink です。project 固有の slug 置換や bare remote 初期化はここに置きません。
- `scripts/`
  - repo-local bootstrap の入口です。
  - template 固有の slug 置換、display name 置換、bare remote 初期化だけをここに置きます。
  - `$start-repository` skill は `scripts/start_repository.sh` を呼び、その wrapper が clean clone では init 前の `make agent-canon-ensure-latest`、`scripts/init_from_template.sh`、必要な post-commit validation をまとめます。`--force` を init に渡すと wrapper preflight は block 扱いで skip し、dirty override を邪魔しません。
- `docker/`
  - Docker runtime profile、runtime pack、notebook profile の定義です。
  - Dockerfile、requirements、pack toml はここに集めます。Codex / GitHub CLI / auth / mount ergonomics は Dockerfile ではなく shared `.devcontainer/` に置きます。
  - Docker を使わない repo では supported runtime の一つとして扱い、日常 validation からは外して構いません。
- `experiments/`
  - experiment profile の実験コード、run ごとの生成物、report を置く場所です。使わないプロジェクトでは空でも構いません。
  - topic 一覧は `experiments/registry.toml`、topic template は `experiments/_template/`、run report は `experiments/report/` に置きます。
- `python/`
  - 実装本体、共通 runtime、テスト対象コードの主置き場です。
- `tests/`
  - pytest ベースのテストを置く場所です。
  - `tests/agent_tools/` と `tests/tools/` は AgentCanon-owned shared-runtime test、`tests/project/` や package-specific tests は project-local implementation test です。

### Bootstrap と Validation の入口

- `make start-repository ARGS='--project-slug your-project --display-name "Your Project"'`
  - clone 直後の推奨入口です。内部で `scripts/start_repository.sh` を呼びます。
- `bash scripts/start_repository.sh --validate-only`
  - init 変更を commit したあと、`agent-canon` submodule pin、fresh clone、quick CI をまとめて確認します。
- `make agent-canon-ensure-latest`
  - AgentCanon update surface が clean で、shared canon / pin 更新が task scope に入る時に `vendor/agent-canon/` submodule pin を configured `agent-canon` remote の `main` と揃えます。
- `make agent-canon-update-plan`
  - 派生 repo から `agent-canon` だけ更新するときの route を read-only で確認します。
- `make agent-canon-update`
  - 派生 repo から `agent-canon` だけ更新します。内部では `update_agent_canon.sh latest` を使う `make agent-canon-latest` と同じ high-level route です。
- `make agent-canon-merge-main`
  - `vendor/agent-canon/` の current branch に GitHub `main` を merge します。派生 repo 側で shared canon を直した branch は、このあと GitHub に push して AgentCanon PR を開きます。
- `make agent-checks`
  - shared surface、skill mirror、agent runtime alignment、research perspective smoke を確認します。
- `make ci-quick`
  - docs、experiment registry、pytest、pyright、pydocstyle を流します。通常の smoke 入口ですが、変更種別に応じた最小 check matrix を優先して構いません。

## 基本方針

- 既定の統合先は `main` です。恒常的な複数 branch 運用はしません。
- 短期 branch は必要なときだけ切り、整理が済んだら `main` に戻します。
- branch 側で file 構成を変えた場合は、`agents/workflows/main-integration-workflow.md` の integration worktree 手順で `main` へ戻します。
- tracked tree に残す durable state は current tree head の canonical path だけです。旧実装、移行用の別経路、`*_old`、`*_copy`、dated snapshot、backup file、古い説明を残した文書を tracked tree に置きません。
- 実装を変えたら、その実装を説明する README、guide、workflow、規約文書も同じ変更で最新実装に合わせます。古い挙動の説明を追記で温存せず、不要になった記述は削除または正本へ置換します。
- 大規模改修、rename、構成変更のあとには、旧実装 path、旧 helper 名、旧文書 path への参照を README、guide、workflow、規約文書、script help から除去し、reader が最新 surface 以外に誘導されない状態までそろえます。
- `documents/` には正本だけを置きます。履歴説明や日付付きの途中報告は置きません。
- 実装変更では、必要なテストと文書更新を同じ変更でそろえます。
- 実験は 1 回の run を fresh 実行として扱い、途中停止 run を正式結果として継ぎ足しません。
- Python の静的解析とテスト、Markdown の体裁とリンク確認は、該当 path を変更した時の日常 check に含めます。
- `psutil`、`pipdeptree`、`deptry`、`snakeviz` は observability / dependency / performance profile の tool です。全 repo の baseline requirement としては扱いません。
- repo-local `.venv` は template default では host に作らず、container 内だけ `python3 tools/ci/python_env_policy.py --create` で `.venv` を許可します。派生 repo が host venv を採用する場合は project-local environment policy で明示します。

shared agent canon は `vendor/agent-canon/` の Git submodule pin として参照します。clone 時は submodule も取得し、root の symlink / copy surface はその pin を runtime view として参照します。ownership と surface 種別は [SHARED_RUNTIME_SURFACES.md](vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md) を正本にし、`.github/workflows/agent-coordination.yml` と `.github/PULL_REQUEST_TEMPLATE/agent_canon.md` は symlink ではなく vendor 正本から同期する root copy として扱います。

## Clone And AgentCanon Update

新規 clone は submodule 込みで取得します。

```bash
git clone --recurse-submodules <template-url> <repo>
cd <repo>
```

submodule なしで clone した場合、または `vendor/agent-canon/` が空の場合は次で復旧します。

```bash
git submodule sync vendor/agent-canon
git submodule update --init --recursive vendor/agent-canon
bash tools/sync_agent_canon.sh check
```

AgentCanon の URL や branch 情報が `.gitmodules` と submodule config でずれた場合は `git submodule sync vendor/agent-canon` を先に実行します。submodule worktree が stale / detached / local-only commit を含む場合は、親 repo の tree diff ではなく `vendor/agent-canon/` の branch / status を確認します。local commit がある branch は `bash tools/update_agent_canon.sh merge-main-into-current` で GitHub `main` を取り込んでから GitHub へ push し、AgentCanon PR にします。

AgentCanon の更新順序は、AgentCanon repo を更新して push / PR merge、template の `vendor/agent-canon` pin 更新、`bash tools/sync_agent_canon.sh link-root`、validation、template commit / push です。`.gitmodules` は template runtime contract の一部なので、AgentCanon URL や branch に関わる PR では必ず確認します。AgentCanon GitHub `main`、template GitHub `origin/main`、submodule pin SHA を PR や closeout で混同しません。

## まず読むもの

- `QUICK_START.md`
- Codex runtime instruction を確認する場合は `AGENTS.md`
- `documents/README.md`
- `vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md`
- clone/bootstrap を触る場合は `documents/template-bootstrap.md`
- agent workflow / skill を確認する場合は `agents/README.md` と `agents/workflows/README.md`
- Python を触る場合は `vendor/agent-canon/documents/coding-conventions-python.md`
- C++ を触る場合は `vendor/agent-canon/documents/cpp-build-layout.md`
- 開発環境を触る場合は `docker/` と `vendor/agent-canon/CONTAINER_OPERATIONS.md`
- host 前提を確認する場合は `documents/linux-wsl-host-requirements.md`
- 実験を行う場合は `agents/workflows/experiment-workflow.md`
- 実験 topic を作る場合は `experiments/README.md`
- topic registry を触る場合は `vendor/agent-canon/documents/experiment-registry.md`

## 日常の進め方

1. 何を変えるかを決めます。実装だけか、実験まで含むか、環境や文書更新が必要かを構造と owner surface から決めます。
1. 変更前に必要な baseline を決めます。docs だけの修正なら status と docs check、Python 変更なら targeted pytest / pyright / ruff、shared canon 変更なら AgentCanon PR gate を選びます。
1. 実装、実験コード、文書、必要なら `docker/` を更新します。
1. 仕上げに changed path と risk class に合った個別チェック、または full confidence が必要なら `make ci` を流します。
1. 長期に残す判断や実験知見は `notes/` に移し、正本ルールは `documents/` に反映します。

## 新規 clone 直後の最短手順

```bash
bash scripts/start_repository.sh --project-slug your-project --display-name "Your Project"
git add -A
git commit -m "chore: initialize project from template"
bash scripts/start_repository.sh --validate-only
```

初期化時の AgentCanon 正本は GitHub submodule です。shared canon の差分は `vendor/agent-canon/` 内の GitHub branch に commit し、AgentCanon PR で戻します。

最短 runbook は `documents/template-bootstrap.md`、notes を育てる方針は `vendor/agent-canon/documents/notes-lifecycle.md` を見ます。

## 実験を含むプロジェクトでの使い方

新規実験は次のような配置を基準にします。

```text
experiments/
├── registry.toml
├── report/
│   └── <run_name>.md
└── <topic>/
    ├── README.md
    ├── cases.*
    ├── experiment.*
    └── result/
        └── <run_name>/
```

- 1 回の run の report は `experiments/report/<run_name>.md`
- run ごとの生成物は `experiments/<topic>/result/<run_name>/`
- 複数 run をまたぐ知見は `notes/experiments/` または `notes/themes/`

実験方法論そのものは `agents/workflows/experiment-workflow.md` と `agents/workflows/research-workflow.md` を正本にします。
agent に実験つき改造 loop を回させる場合は `agents/skills/adaptive-improvement-loop.md` を outer loop、`agents/skills/experiment-lifecycle.md` を run 単位の分岐に使います。
server で回す実験コードの実体テンプレは `experiments/_template/`、topic 正本は `experiments/registry.toml`、topic scaffold は `tools/experiments/create_experiment_topic.py`、run metadata を残す入口は `tools/experiments/run_managed_experiment.py` です。

## よく使うコマンド

```bash
make check-matrix
make docs-check
python3 -m pytest tests/ -q --tb=short
python3 -m pyright
python3 -m ruff check python tests --select D,E,F,I,UP
make agent-canon-update-plan
make agent-canon-pr-check
make docker-check
make docker-build-check
make experiment-check
```

`make check-matrix` は task に合う check を選ぶための短い表です。
`make clean-generated` は ignored な `build/`、`logs/`、`reports/`、pytest / ruff cache、`__pycache__`、devcontainer generated compose だけを消します。template として残す tracked product file は消しません。

## Docker で Codex を使う

AgentCanon を持つ repo の container / devcontainer 境界は
[CONTAINER_OPERATIONS.md](vendor/agent-canon/CONTAINER_OPERATIONS.md) を先に見ます。
`docker/Dockerfile` は project runtime、shared `.devcontainer/` は Codex / GitHub CLI /
host mount などの agent ergonomics を持ちます。template 固有の実装 runbook は
[docker/README.md](docker/README.md) です。

Jupyter notebook runtime は notebook profile です。host browser から使う場合は `make docker-jupyter` を実行し、runner が `docker/install_python_dependencies.sh` を通してから JupyterLab を起動します。既定 token は local development 用の例で、shared host では `JUPYTER_TOKEN` を明示してください。host 側では template default として repo-local `.venv` を作らず、devcontainer や nested Codex など container 内でだけ `make python-env-status` と `make python-env-prepare` を使って `.venv` を用意します。

Dockerfile、requirements、Python installer、runtime pack のいずれかを変えたら
`bash tools/docker_dependency_validator.sh` を先に通します。image build や pack smoke に
影響する変更では `make docker-build-check` も通します。ローカルに `docker` / `podman` が
ない場合は、GitHub Actions の `Docker Build` workflow を使います。

repo-wide な tool 導入案や Docker 変更では `agents/templates/environment_change_proposal.md` に triggering code requirement、blocked command、Docker 影響、validation、rollback を残します。

project-scoped Codex config の正本は `.codex/config.toml` です。template 既定では `approval_policy = "never"` と `sandbox_mode = "danger-full-access"` を入れているので、container 内で起動した Codex も最初から full access 前提です。

VS Code の dev container は `.devcontainer/` から起動します。compose 生成、mount、
auth reuse、post-create、attach status の詳細は `CONTAINER_OPERATIONS.md` と
`docker/README.md` に寄せます。

container 内では `PYTHONPATH=/workspace/python` を前提にします。
C++ を使うときの canonical entrypoint は root [CMakeLists.txt](CMakeLists.txt) です。helper module は [cmake/README.md](cmake/README.md)、layout と artifact reuse policy は [cpp-build-layout.md](vendor/agent-canon/documents/cpp-build-layout.md) を見ます。

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
```

container 内から `docker build` / `docker run` を行う場合は、上のように host の Docker socket を渡すか、別 daemon を用意します。

build 確認だけを行う場合は次です。

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

## 詳細入口

- 規約と運用: `documents/README.md`
- 補助メモ: `notes/README.md`
- エージェント運用: `agents/README.md`
- shared automation: `tools/README.md`
- repo-local bootstrap: `scripts/README.md`

## License

This template repository is licensed under Apache License 2.0. See
[LICENSE](LICENSE) and [documents/licensing-policy.md](documents/licensing-policy.md).

Derived repositories may choose their own project license by replacing the root
`LICENSE` and package metadata. The AgentCanon submodule remains licensed by its
own [vendor/agent-canon/LICENSE](vendor/agent-canon/LICENSE), and root symlink
views into AgentCanon keep that upstream license boundary.
