# ptymark 開発環境

## 二つの環境面

このrepositoryには二つの環境面があります。

1. project-template / AgentCanonのローカル作業環境
2. ptymark製品のcanonical Docker環境

前者は既存の`docker/Dockerfile`、runtime packs、`.devcontainer/`、Python toolchainを
維持します。後者は`docker/ptymark.Dockerfile`と
`docker/ptymark-compose.yaml`で独立に固定します。

ptymarkの正式なproduct checkでは後者を使います。

## Host requirements

- Git
- Docker EngineまたはDocker Desktop
- Docker Compose v2
- 実際のWezTerm統合を確認する場合だけWezTerm
- macOS native releaseをローカル作成する場合だけRust toolchain

```bash
docker version
docker compose version
```

ptymark containerへhost Docker socketをmountしません。

## Initial setup

```bash
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
make ptymark-docker-build
make ptymark-check
```

`make ptymark-docker-build`:

- `docker/ptymark-versions.env`を読み込む
- `docker/ptymark.Dockerfile`をbuildする
- Mermaid CLI、KaTeX、Typst、Chromium、Rust、Luaを固定する

`make ptymark-check`:

- imageをbuildする
-同じCompose serviceで`make ptymark-check-local`を実行する

`ptymark-check-local`は`/.dockerenv`がない環境では失敗します。
正式なproduct validationへhost依存が混ざることを防ぎます。

## Daily development

interactive shell:

```bash
make ptymark-dev
```

single command:

```bash
./scripts/ptymark-dev-container.sh cargo test --locked --all-targets
./scripts/ptymark-dev-container.sh cargo clippy --locked --all-targets -- -D warnings
./scripts/ptymark-dev-container.sh lua5.4 tests/plugin_smoke.lua
./scripts/ptymark-dev-container.sh bash scripts/check-ptymark-renderers.sh
```

Compose mount:

- repository: `/workspace`
- Cargo registry: named volume
- Cargo git checkout: named volume
- target: named volume under `/home/node/.cache/ptymark-target`

cache reset:

```bash
make ptymark-clean
make ptymark-docker-build
make ptymark-check
```

## Test layers

### Rust unit tests

- semantic detector chunk boundaries
- unclosed/over-limit fallback
- preview/source/external renderer
- external process timeout/output limit
- pre-display ordering/fallback/bypass
- viewport resize decision
- render key generation
- bounded LRU cache

### Rust integration tests

- ordinary bytes are unchanged
- completed semantic source is replaced before stdout display
- chunk split does not change final output
- renderer error fallback and strict failure
- CLI help/version/preview/command exit status

### Plugin smoke

`tests/plugin_smoke.lua`はWezTerm API stubを使い、次を検証します。

- default command
- explicit command
- launch menu append
- key append
- cwd/environment propagation
- opt-out behavior

実WezTerm GUIはDockerへ入れません。最終統合だけhost WezTermで確認します。

### Existing engine smoke

`scripts/check-ptymark-renderers.sh`:

- Mermaid CLIでSVG生成
- KaTeXでTeXからMathML生成
- TypstでSVG生成

これはengine availabilityとversioned environmentを検証します。Rustコア内でそれらの
layout engineを再実装しません。

## Host WezTerm integration

1. host OS用`ptymark`をbuild/installする。
2. local `file://` plugin URLを設定する。
3. `binary`にhost binary pathを指定する。
4. launch menu/key bindingからtabを起動する。
5. input、Ctrl+C、child exit、resizeを確認する。

```lua
local ptymark = wezterm.plugin.require(
  'file:///absolute/path/to/ptymark'
)

ptymark.apply_to_config(config, {
  binary = '/absolute/path/to/ptymark',
})
```

現alphaのcommand modeは透過`exec`です。PTY/display integration実装後は同じmanual
smokeへsemantic rendering、alternate screen bypass、resize generationを追加します。

## Product targetとtemplate target

通常の`make`はroot `GNUmakefile`を読みます。

```text
GNUmakefile
├── include Makefile      # existing template / AgentCanon targets
└── include ptymark.mk    # ptymark product targets
```

したがって同じrepositoryで次を併用できます。

```bash
make agent-checks
make ci-quick
make docker-build-check
make ptymark-check
```

AgentCanon/root views/template profilesをproduct bootstrapのために削除しません。

## Version update

```bash
$EDITOR docker/ptymark-versions.env
$EDITOR rust-toolchain.toml Cargo.toml
$EDITOR docker/ptymark.Dockerfile docker/ptymark-compose.yaml

make ptymark-clean
docker compose \
  --env-file docker/ptymark-versions.env \
  --file docker/ptymark-compose.yaml \
  build --pull --no-cache
make ptymark-check
```

同じ変更で`documents/dependencies.md`、README、必要に応じて`Cargo.lock`を更新します。
