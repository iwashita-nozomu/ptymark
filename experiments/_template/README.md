# Experiment Topic Template
<!--
@dependency-start
contract template
responsibility Documents Experiment Topic Template for this repository.
upstream design ../README.md experiments hub guidance
downstream implementation cases.py case scaffold
downstream implementation run.py run scaffold
@dependency-end
-->

このディレクトリは、新しい experiment topic を始めるための最小雛形です。
server 上でそのまま回せるように、`cases.py`、`run.py`、`config.yaml`、`notebooks/`、`result/` の入口をそろえています。

## この文書の読み方

- この template は、新しい experiment topic の question、comparison target、notebook、logs、standard commands、expected outputs、notes の入口をそろえます。
- Question と Comparison Target で目的を固定し、Visualization Notebook、Logs、Standard Commands、Expected Outputs、Notes で run と artifact の形を確認します。
- `_template/` から topic を作った直後、managed runner の smoke / formal command、生成物、registry entry を埋めるときに読みます。
- Topic 固有の仮説や結果はこの template に残さず、作成した topic の README、result、report に移します。

## Question

<!-- この topic が何を確かめる実験なのかを書く -->

## Comparison Target

<!-- main、baseline、reference method などを書く -->

## Visualization Notebook

可視化は `notebooks/` 配下の Jupyter notebook に置きます。Notebook は
`result/<run_name>/summary.json`、`cases.jsonl`、必要な `logs/` artifact を読み、
figure / table を作る入口です。formal run の起動や設定正本にはしません。

## Logs

各 run は `result/<run_name>/logs/` を持ちます。top-level の `run.log` は
managed runner の統合ログとして残します。runner は `logs/startup.jsonl`、
`logs/stdout.log`、`logs/stderr.log` を作り、topic 固有の tool log と diagnostic
log も `logs/` 配下へ置きます。

## Standard Commands

smoke:

```bash
python3 tools/experiments/run_managed_experiment.py \
  --topic <topic> \
  --use-registered-command smoke
```

formal run:

```bash
python3 tools/experiments/run_managed_experiment.py \
  --topic <topic> \
  --use-registered-command formal
```

## Expected Outputs

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
- `notebooks/<run_name>.ipynb` またはこの topic で固定した可視化 notebook
- `experiments/report/<run_name>.md`

## Notes

- `cases.py` は case 列の定義に集中させます。
- `run.py` は CLI、orchestration、summary 出力に集中させます。
- `experiments/registry.toml` に topic entry を追加し、`smoke_inner_command` と `formal_inner_command` を正本にします。
- registered command には `{config_path}` を含め、実験 script は `--config <path>` から JSON object を読みます。
- `config.yaml` は managed runner の起動前提です。`source_snapshot.json` は topic source、registry、command source、runner source、dirty source file の digest と git status を持ちます。
- 追加で自動収集したい eval artifact がある場合は、`experiments/registry.toml` の `required_eval_artifacts` / `optional_eval_artifacts` に topic 固有 artifact の pattern を書きます。managed file と managed log は `artifact_manifest.json` で辿ります。
- formal run では `run_name` と protocol を固定した fresh 実行にします。
