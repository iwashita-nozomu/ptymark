PTYMARK_COMPOSE = docker compose --env-file docker/ptymark-versions.env --file docker/ptymark-compose.yaml

.PHONY: ptymark-build ptymark-check ptymark-check-local ptymark-dev ptymark-clean

ptymark-build:
	$(PTYMARK_COMPOSE) build --pull

ptymark-check: ptymark-build
	$(PTYMARK_COMPOSE) run --rm --no-TTY dev make ptymark-check-local

ptymark-check-local:
	@test -f /.dockerenv || { echo "ptymark-check-local must run in Docker" >&2; exit 1; }
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked --all-targets
	cargo build --locked
	lua5.4 tests/plugin_smoke.lua
	bash -n scripts/check-ptymark-renderers.sh scripts/installer.sh scripts/install.sh scripts/install-managed-bundle.sh tests/install_smoke.sh
	shellcheck scripts/check-ptymark-renderers.sh scripts/installer.sh scripts/install.sh scripts/install-managed-bundle.sh tests/install_smoke.sh
	node --check renderers/managed/mathjax-cli.mjs
	node --check renderers/managed/ansi-presenter.mjs
	bash tests/install_smoke.sh
	bash scripts/check-ptymark-renderers.sh

ptymark-dev:
	$(PTYMARK_COMPOSE) run --rm dev bash

ptymark-clean:
	$(PTYMARK_COMPOSE) down --volumes --remove-orphans
