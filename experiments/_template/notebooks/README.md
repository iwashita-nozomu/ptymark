# Visualization Notebooks
<!--
@dependency-start
contract template
responsibility Documents Experiment Topic Notebook Template for this repository.
upstream design ../README.md experiment topic template contract
@dependency-end
-->

このディレクトリには、run artifact を可視化する Jupyter notebook を置きます。
Notebook は `../result/<run_name>/summary.json`、`cases.jsonl`、必要な
`logs/` artifact を読み、figure / table を再生成できる形にします。

formal run の起動、細かな test、設定正本は notebook に置きません。
