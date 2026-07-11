# Notes Hub
<!--
@dependency-start
contract design
responsibility Documents Notes Hub for this repository.
upstream design ../vendor/agent-canon/documents/notes-lifecycle.md note lifecycle contract
downstream design ../memory/README.md stable memory promotion target
@dependency-end
-->

`notes/` は、この template で長く残したい知見、比較、補助メモの置き場です。
規約や設計の一次情報は `documents/` に残し、ここではそれに昇格させる前の知見や、run をまたいで残したい判断を扱います。

## カテゴリ

- `experiments/`
  - 個別実験をまたぐ要約、比較、carry-over
- `themes/`
  - 複数実験や調査から得た話題別の知見
- `knowledge/`
  - 実務で繰り返し参照する横断的な短い知識メモ
- `branches/`
  - branch 例外運用時の要約と入口
- `worktrees/`
  - worktree action log と carry-over
- `guardrails/`
  - avoid pattern と実務 guardrail
- `failures/`
  - 再発防止の短い記録

## 使い方

- 規約へ昇格する内容は `documents/` に移します。
- branch / worktree の action log は closeout 時に `knowledge`、`themes`、`failures` へ昇格させます。
- cross-repo に効く durable な user preference は shared canon の `memory/USER_PREFERENCES.md` に記録し、安定後に `AGENTS.md` への昇格を判断します。
- repo-specific な theme や調査メモは `notes/themes/` に置きます。`notes/themes/USER_PREFERENCES.md` は新規の正本として作りません。

## 参照先

- `documents/notes-lifecycle.md`
- `documents/worktree-lifecycle.md`
