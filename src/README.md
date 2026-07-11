# src
<!--
@dependency-start
contract design
responsibility Documents src for this repository.
upstream design ../cmake/README.md CMake layout guidance
@dependency-end
-->

template 既定では C++ 実装を持ちません。
派生 repo で C++ を使う場合は、まず `include/<project>/*.hpp` に public header を置き、必要なときだけ `src/` に translation unit を追加します。

`src/` を使うのは次のような特例だけです。

- header-only だと compile time や ODR の負担が大きすぎる
- 外部 library の binary link が必要
- 明示的な translation unit 分離が必要

`src/` に実装を置く場合は、なぜ header-only では駄目かを change note か設計文書に残します。
