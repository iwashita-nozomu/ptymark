# Experiments Hub
<!--
@dependency-start
contract design
responsibility Documents Experiments Hub for this repository.
upstream design ../vendor/agent-canon/documents/experiment-registry.md experiment registry contract
downstream environment registry.toml template-local registry
@dependency-end
-->

`experiments/` は、server 上で回す実験コード、run ごとの生成物、1 run ごとの report をまとめる場所です。
この template では、topic ごとの実験コードと run artifact を同じ tree に寄せます。
shared canon では、このうち topic 共通の scaffold と report 導線だけを保持し、派生 repo ごとの `registry.toml` と `experiments/<topic>/` は root 側の正本に残します。

## この文書の読み方

- この文書は、root `experiments/` の標準構成、topic 作成、server 実行、registry check、context sync の入口を説明します。
- 前半は標準構成と最初に使う tool、server 実行の生成物、topic の作り始めを扱い、後半は実行例、registry check、context sync を扱います。
- 新しい experiment topic を作るとき、formal run の置き場所を確認するとき、または registry と branch / worktree context を同期するときに読みます。
- 派生 repo ごとの `registry.toml` と `experiments/<topic>/` が root 側の正本であり、shared canon は共通 scaffold と report 導線だけを持ちます。

## 標準構成

```text
experiments/
├── registry.toml
├── report/
│   ├── README.md
│   └── <run_name>.md
└── <topic>/
    ├── README.md
    ├── cases.py
    ├── config.yaml
    ├── notebooks/
    ├── run.py
    └── result/
        └── <run_name>/
            └── logs/
```

## まず使うもの

- `_template/`
  - 新しい topic を始めるときの最小雛形です。
- `registry.toml`
  - topic、entrypoint、formal run command、active branch の集中管理ファイルです。
- `report/README.md`
  - run report の置き方です。
- `tools/experiments/create_experiment_topic.py`
  - `_template/` から新しい topic を作り、registry entry も追加します。
- `tools/experiments/sync_experiment_registry_context.py`
  - current branch / worktree / scope file を registry に同期します。
- `tools/experiments/run_managed_experiment.py`
  - `run_manifest.json`、`eval_manifest.json`、`artifact_manifest.json`、`command.json`、`environment.json`、`source_snapshot.json`、`config.json`、`config_source.yaml`、`run.log`、run ごとの `logs/` を残しながら実験を実行する入口です。

## server 実行の既定

- fresh run は 1 つの `run_name` と 1 つの `result/<run_name>/` に閉じます。
- server 上の formal run は `run_managed_experiment.py` 経由で実行します。
- formal run でどの code を実行するかは `registry.toml` の `formal_inner_command` を正本にします。
- 主要生成物は次を基準にします。
  - `result/<run_name>/run_manifest.json`
  - `result/<run_name>/eval_manifest.json`
  - `result/<run_name>/artifact_manifest.json`
  - `result/<run_name>/command.json`
  - `result/<run_name>/environment.json`
  - `result/<run_name>/source_snapshot.json`
  - `result/<run_name>/config.json`
  - `result/<run_name>/config_source.yaml`
  - `result/<run_name>/run.log`
  - `result/<run_name>/logs/startup.jsonl`
  - `result/<run_name>/logs/stdout.log`
  - `result/<run_name>/logs/stderr.log`
  - `result/<run_name>/summary.json`
  - `result/<run_name>/cases.jsonl`
- 可視化は `experiments/<topic>/notebooks/` の Jupyter notebook に置きます。Notebook は run artifact を読んで図表化する入口で、正式 run の起動や設定正本にはしません。
- 1 回の run report は `experiments/report/<run_name>.md` に置きます。

## topic の作り始め

```bash
python3 tools/experiments/create_experiment_topic.py <topic>
```

コピーしたら、少なくとも次をその topic に合わせて書き換えます。

- `README.md`
- `cases.py`
- `config.yaml`
- `notebooks/`
- `run.py`
- `registry.toml` の topic entry
- 標準コマンド
- `Question:`
- `Comparison Target:`
- `Visualization Notebook:`
- `Logs:`

## 実行例

```bash
python3 tools/experiments/run_managed_experiment.py \
  --topic _template \
  --use-registered-command smoke
```

この wrapper は、`registry.toml` の topic entry を見て command 実行前に result dir、`logs/`、`config.json`、`config_source.yaml`、`command.json`、`environment.json`、`source_snapshot.json`、report stub を初期化し、起動順序を `logs/startup.jsonl` に残します。終了後に `run_manifest.json` と `artifact_manifest.json` を更新し、`summary.json` / `cases.jsonl` / `config.json` と registry で指定した topic 固有 eval artifact を `eval_manifest.json` に収集します。managed file と managed log は `artifact_manifest.json` で辿ります。

`source_snapshot.json` は topic source、registry、command source、runner source、dirty source file の digest と git status を持ちます。`environment.json` は full environment を key 名ベースで redaction して保存します。

## Registry Check

```bash
python3 tools/ci/check_experiment_registry.py
make experiment-check
```

## Context Sync

branch / worktree を使う場合は、scope 更新後に次で registry metadata を合わせます。

```bash
python3 tools/experiments/sync_experiment_registry_context.py \
  --topic <topic> \
  --branch work/<topic>-YYYYMMDD \
  --workspace-root .worktrees/<branch-name>
```
