# CMake Layout
<!--
@dependency-start
contract design
responsibility Documents CMake Layout for this repository.
downstream implementation ../CMakeLists.txt CMake entrypoint
downstream design ../src/README.md C++ source layout guidance
@dependency-end
-->

この template で C++ を使うときの CMake 正本は root の `CMakeLists.txt` です。

- root `CMakeLists.txt`
  - repo 全体の canonical entrypoint
- `cmake/`
  - `find_package` 補助や toolchain helper を project が必要になったときに置く場所
- `include/`
  - 派生 repo が C++ を使う場合の public header
- `src/`
  - header-only で収まらない場合の特例実装
- `lib/`
  - checked-in third-party source や手動 vendor する補助 library
- `tests/cpp/`
  - project が C++ を使う場合の test と smoke source

build は必ず out-of-source で行います。
既定の build tree は `build/cpp/<profile>/`、再利用する local install tree は `.state/cpp-install/<profile>/` です。

template の canonical C++ target は空の `INTERFACE` library です。
派生 repo が C++ を使うときは、この target に project 固有の include、source、test target を足します。
