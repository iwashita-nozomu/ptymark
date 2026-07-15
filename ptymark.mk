PTYMARK_COMPOSE = docker compose --env-file docker/ptymark-versions.env --file docker/ptymark-compose.yaml
PTYMARK_BASH_SCRIPTS = \
	scripts/check-ptymark-renderers.sh \
	scripts/installer.sh \
	scripts/install.sh \
	scripts/install-managed-bundle.sh \
	scripts/package-release.sh \
	distribution/install.sh \
	tests/install_smoke.sh \
	tests/managed_renderer_smoke.sh \
	tests/shell_profile_coexistence.sh

.PHONY: ptymark-build ptymark-check ptymark-check-local ptymark-runtime-dependencies
.PHONY: ptymark-verify-catalog ptymark-dev ptymark-clean

ptymark-build:
	$(PTYMARK_COMPOSE) build --pull

ptymark-check: ptymark-build
	$(PTYMARK_COMPOSE) run --rm --no-TTY dev make ptymark-check-local

ptymark-runtime-dependencies:
	node scripts/check-ptymark-runtime-dependencies.mjs

ptymark-verify-catalog:
	cargo test --locked --test verification_manifest_contract

ptymark-check-local:
	@test -f /.dockerenv || { echo "ptymark-check-local must run in Docker" >&2; exit 1; }
	$(MAKE) ptymark-runtime-dependencies
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked --all-targets
	cargo build --locked
	lua5.4 tests/plugin_smoke.lua
	bash -n $(PTYMARK_BASH_SCRIPTS)
	shellcheck $(PTYMARK_BASH_SCRIPTS)
	node --check renderers/managed/mathjax-cli.mjs
	node --check renderers/managed/ansi-presenter.mjs
	bash tests/install_smoke.sh
	bash tests/shell_profile_coexistence.sh "$${CARGO_TARGET_DIR:-target}/debug/ptymark"
	PTYMARK_TEST_BROWSER=/usr/bin/chromium PTYMARK_BROWSER_NO_SANDBOX=1 bash tests/managed_renderer_smoke.sh
	bash scripts/check-ptymark-renderers.sh

ptymark-dev:
	$(PTYMARK_COMPOSE) run --rm dev bash

ptymark-clean:
	$(PTYMARK_COMPOSE) down --volumes --remove-orphans
