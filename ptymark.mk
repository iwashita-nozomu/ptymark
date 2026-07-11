PTYMARK_COMPOSE = docker compose --env-file docker/ptymark-versions.env --file docker/ptymark-compose.yaml

.PHONY: ptymark-docker-build ptymark-check ptymark-check-local ptymark-benchmark ptymark-dev ptymark-clean

ptymark-docker-build:
	$(PTYMARK_COMPOSE) build --pull

ptymark-check: ptymark-docker-build
	$(PTYMARK_COMPOSE) run --rm --no-TTY dev make ptymark-check-local

ptymark-check-local:
	@test -f /.dockerenv || { echo "ptymark-check-local must run in the canonical Docker environment" >&2; exit 1; }
	cargo metadata --locked --format-version 1 >/dev/null
	cargo fmt --all -- --check
	cargo clippy --locked --all-targets -- -D warnings
	cargo test --locked --all-targets
	cargo build --locked --release
	cargo run --quiet --locked -- demo > /tmp/ptymark-demo.txt
	grep -F "ptymark mermaid preview" /tmp/ptymark-demo.txt >/dev/null
	cargo run --quiet --locked -- config check --config examples/ptymark.example.toml
	cargo run --quiet --locked -- config show --config examples/ptymark.example.toml --profile private > /tmp/ptymark-config.toml
	grep -F 'private = true' /tmp/ptymark-config.toml >/dev/null
	cargo run --quiet --locked -- --config examples/ptymark.example.toml --profile private -- /bin/sh -c 'exit 0'
	lua5.4 tests/plugin_smoke.lua
	bash -n scripts/ptymark-dev-container.sh scripts/check-ptymark-dependencies.sh scripts/check-ptymark-renderers.sh scripts/benchmark-ptymark-renderers.sh scripts/package-ptymark-release.sh
	shellcheck scripts/ptymark-dev-container.sh scripts/check-ptymark-dependencies.sh scripts/check-ptymark-renderers.sh scripts/benchmark-ptymark-renderers.sh scripts/package-ptymark-release.sh
	python3 -m py_compile scripts/check-ptymark-benchmarks.py
	node --check "$$PTYMARK_RENDERER_ROOT/worker.mjs"
	node --check "$$PTYMARK_RENDERER_ROOT/check.mjs"
	node --check "$$PTYMARK_RENDERER_ROOT/benchmark.mjs"
	bash scripts/check-ptymark-dependencies.sh
	bash scripts/check-ptymark-renderers.sh
	rm -rf /tmp/ptymark-dist
	bash scripts/package-ptymark-release.sh target/release/ptymark /tmp/ptymark-dist
	test -n "$$(find /tmp/ptymark-dist -name 'ptymark-*.tar.gz' -print -quit)"

ptymark-benchmark:
	$(PTYMARK_COMPOSE) run --rm --no-TTY dev bash scripts/benchmark-ptymark-renderers.sh

ptymark-dev:
	$(PTYMARK_COMPOSE) run --rm dev bash

ptymark-clean:
	$(PTYMARK_COMPOSE) down --volumes --remove-orphans
