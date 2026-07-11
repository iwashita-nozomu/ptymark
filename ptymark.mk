PTYMARK_COMPOSE = docker compose --env-file docker/ptymark-versions.env --file docker/ptymark-compose.yaml

.PHONY: ptymark-docker-build ptymark-check ptymark-check-local ptymark-dev ptymark-clean

ptymark-docker-build:
	$(PTYMARK_COMPOSE) build --pull

ptymark-check: ptymark-docker-build
	$(PTYMARK_COMPOSE) run --rm --no-TTY dev make ptymark-check-local

ptymark-check-local:
	@test -f /.dockerenv || { echo "ptymark-check-local must run in the canonical Docker environment" >&2; exit 1; }
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked --all-targets
	cargo build --locked --release
	cargo run --quiet --locked -- demo > /tmp/ptymark-demo.txt
	grep -F "ptymark mermaid preview" /tmp/ptymark-demo.txt >/dev/null
	lua5.4 tests/plugin_smoke.lua
	bash -n scripts/ptymark-dev-container.sh
	bash -n scripts/check-ptymark-dependencies.sh
	bash -n scripts/check-ptymark-renderers.sh
	bash -n scripts/package-ptymark-release.sh
	shellcheck scripts/ptymark-dev-container.sh scripts/check-ptymark-dependencies.sh scripts/check-ptymark-renderers.sh scripts/package-ptymark-release.sh
	bash scripts/check-ptymark-dependencies.sh
	bash scripts/check-ptymark-renderers.sh
	rm -rf /tmp/ptymark-dist
	bash scripts/package-ptymark-release.sh target/release/ptymark /tmp/ptymark-dist
	test -n "$$(find /tmp/ptymark-dist -name 'ptymark-*.tar.gz' -print -quit)"

ptymark-dev:
	$(PTYMARK_COMPOSE) run --rm dev bash

ptymark-clean:
	$(PTYMARK_COMPOSE) down --volumes --remove-orphans
