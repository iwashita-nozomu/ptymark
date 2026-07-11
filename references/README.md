# References
<!--
@dependency-start
contract design
responsibility Documents References for this repository.
upstream design ../AGENTS.md reference sweep requirement
@dependency-end
-->

このディレクトリは、実装、実験、workflow 設計で参照した一次資料や索引を置く場所です。
topic ごとの論文束や reference note をまとめる場合は、ここを入口にします。

## 置くもの

- topic ごとの reference index
- 論文、標準、仕様書、手順書の整理メモ
- repo-wide workflow や review policy の外部根拠に紐づく補助資料

## 置かないもの

- 日付付きの作業ログ
- run ごとの一次結果
- repo-wide の恒久ルールそのもの

これらは次へ分けます。

- 作業ログや補助メモ
  - `notes/`
- run ごとの結果や report
  - `experiments/` または `reports/agents/`
- 恒久ルール
  - `documents/` と `agents/`

## Source Record Policy

Before adding a new source note, search existing `references/`, `notes/`,
`documents/`, and task reports for the same title, DOI, URL, or claim. If an
existing note already covers the source, update or cite that note instead of
creating a duplicate.

When an external source is used in an answer, design, workflow, experiment, or
review, leave a durable source record. At minimum record the URL or DOI,
access date, claim used, known limitation, adoption or exclusion decision, and
whether a downloaded artifact was retained or intentionally left outside the
tracked tree.

## 推奨構成

    references/
    ├── README.md
    ├── workflow/
    │   └── README.md
    └── <topic>/
        ├── README.md
        └── *.pdf

## 関連入口

- [agents/workflows/README.md](../agents/workflows/README.md)
  - workflow と review policy の正本入口です。
- [agents/workflows/research-workflow.md](../agents/workflows/research-workflow.md)
  - 研究・実験改造の workflow 正本です。
- [notes/README.md](../notes/README.md)
  - cross-run の知見整理はこちらです。
