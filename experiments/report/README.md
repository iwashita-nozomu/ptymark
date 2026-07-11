# Experiment Reports
<!--
@dependency-start
contract design
responsibility Documents Experiment Reports for this repository.
upstream design ../README.md experiments hub guidance
upstream design ../../vendor/agent-canon/documents/experiment-report-style.md report style contract
@dependency-end
-->

`experiments/report/` には 1 回の run に対応する report を置きます。
report 名は `run_name` とそろえ、`experiments/<topic>/result/<run_name>/` と 1 対 1 で辿れるようにします。

最低限、次を report 側で辿れるようにします。

- `Question:`
- `Comparison Target:`
- exact command
- `result/<run_name>/` の path
- `run_manifest.json`
- `eval_manifest.json`
- `summary.json`
- `cases.jsonl`
- `result/<run_name>/logs/`
- 可視化 notebook

cross-run の要約や campaign 全体の知見は `notes/experiments/` へ移します。
