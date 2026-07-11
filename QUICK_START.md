# Quick Start

<!--
@dependency-start
contract design
responsibility Provides the shortest ptymark product and retained template/AgentCanon setup routes.
upstream design README.md primary repository overview
upstream design AGENTS.md runtime agent entrypoint
upstream design documents/development-environment.md ptymark Docker environment
upstream design vendor/agent-canon/CONTAINER_OPERATIONS.md AgentCanon container rulebook
@dependency-end
-->

## ptymarkを利用する

### Source install

```bash
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
cargo install --locked --path .
ptymark --version
ptymark demo
```

### WezTerm plugin

`~/.wezterm.lua`:

```lua
local wezterm = require 'wezterm'
local config = wezterm.config_builder()

local ptymark = wezterm.plugin.require(
  'https://github.com/iwashita-nozomu/ptymark'
)

ptymark.apply_to_config(config, {
  binary = 'ptymark',
})

return config
```

現在のcommand modeは透過`exec`です。表示前rendererの動作確認:

```bash
cat <<'EOF' | ptymark preview
before
$$
E = mc^2
$$
after
EOF
```

詳細は[利用方法](documents/usage.md)を参照してください。

## ptymarkを開発する

ホストに必要なのはGit、Docker、Docker Compose v2です。

```bash
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
make ptymark-docker-build
make ptymark-check
```

interactive development shell:

```bash
make ptymark-dev
```

single check:

```bash
./scripts/ptymark-dev-container.sh cargo test --locked --all-targets
./scripts/ptymark-dev-container.sh lua5.4 tests/plugin_smoke.lua
./scripts/ptymark-dev-container.sh bash scripts/check-ptymark-renderers.sh
```

product docs:

- [基本設計](documents/architecture.md)
- [UI設計](documents/ui-design.md)
- [依存関係](documents/dependencies.md)
- [開発環境](documents/development-environment.md)
- [配布](documents/distribution.md)

## Template / AgentCanonローカル作業

project-template構造、AgentCanon submodule、root views、Python/C++/experiment profilesは
残しています。`GNUmakefile`が既存`Makefile`と`ptymark.mk`をincludeするため、通常の
`make`から両方のtargetを使えます。

最初に読む:

- [documents/README.md](documents/README.md)
- [Runtime Profiles And Check Matrix](vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md)
- [Agent workflow hub](agents/README.md)
- [Template Bootstrap](documents/template-bootstrap.md)

基本確認:

```bash
git status --short
make check-matrix
make agent-canon-status
```

代表target:

```bash
make ci-quick
make docs-check
make agent-checks
make agent-canon-pr-check
make docker-build-check
make experiment-check
make ptymark-check
```

AgentCanon source/root viewを触る場合:

- `vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md`
- `vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md`
- `agents/workflows/agent-canon-pr-workflow.md`

を先に確認します。

## Repositoryの終了時整理

```bash
git status --short
git branch --show-current
bash tools/push_origin.sh
```

- project固有のactive contractはroot `documents/`へ置く。
- shared AgentCanon ruleは`vendor/agent-canon/`側の正本へ反映する。
- ptymark product changeではRust tests、plugin smoke、Docker renderer smokeをそろえる。
- 長期知識は`notes/`、実験成果は`experiments/`、run artifactは`reports/`へ置く。
