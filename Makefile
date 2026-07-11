# @dependency-start
# contract configuration
# responsibility Defines template make targets for validation, setup, and agent workflow automation.
# upstream implementation tools/agent_tools/evaluate_agent_run.py exposes agent-evaluate target
# upstream implementation tools/agent_tools/task_close.py enforces closeout gates
# upstream implementation tools/agent_tools/run_repo_dependency_review.sh exposes repo-wide dependency review
# upstream implementation tools/agent_tools/review_backlog_scan.sh exposes integrated review backlog scans
# @dependency-end

PYTHON ?= python3

AGENT_TOOLS := tools/agent_tools
CI_TOOLS := tools/ci
AGENT_CANON_SYNC := bash tools/sync_agent_canon.sh
AGENT_CANON_UPDATE := bash tools/update_agent_canon.sh

DOCKER_DEFAULT_PACK ?= docker/packs/default.toml
DOCKER_HOST_PACK ?= docker/packs/default-host-docker.toml
SERVER_LAYOUT ?= vendor/agent-canon/documents/templates/server_runtime_layout.template.toml
REPO_WIDE_REVIEW_REPORT_DIR ?= reports/agents/repo-wide-review-check
REPO_WIDE_REVIEW_QUERY ?= repo-wide review runtime surface stale path check

.PHONY: ci ci-quick check-matrix docs-check clean-generated github-workflow-check
.PHONY: fresh-clone-check template-check dev-setup tools-help
.PHONY: start-repository task-start doc-start task-close agent-evaluate
.PHONY: dependency-review dependency-review-surfaces review-backlog-scan waterfall-gate-check
.PHONY: user-preference-log
.PHONY: agent-checks agent-surface-checks
.PHONY: repo-wide-review-check semantic-index-stale-check
.PHONY: agent-canon-check agent-canon-latest-check agent-canon-links agent-canon-status
.PHONY: agent-canon-ensure-latest agent-canon-rebuild-tools agent-canon-update-plan
.PHONY: agent-canon-latest agent-canon-update agent-canon-merge-main agent-canon-pr-check
.PHONY: docker-check python-env-status python-env-prepare
.PHONY: docker-build-check docker-build-check-host-docker docker-run devcontainer-render
.PHONY: server-check experiment-check docker-shell docker-jupyter docker-codex docker-codex-host-docker

# Validation targets
# Full confidence gate: agent/runtime, docs, Rust, container, pytest, pyright,
# pydocstyle, and ruff. Use check-matrix for targeted day-to-day validation.
ci:
	bash tools/ci/run_all_checks.sh

# Broad gate with ruff skipped; still runs the other full-confidence surfaces.
ci-quick:
	bash tools/ci/run_all_checks.sh --quick

# changed-path / profile based check selector
check-matrix:
	@echo "Check matrix:"
	@echo "  docs-only:        make docs-check && dependency header checks for changed docs"
	@echo "  Python changes:   targeted pytest + python3 -m pyright + python3 -m ruff check python tests --select D,E,F,I,UP"
	@echo "  AgentCanon source:   make agent-canon-pr-check (includes broad quick CI with duplicate docs/workflow gates skipped)"
	@echo "  AgentCanon shared views: make agent-canon-check"
	@echo "  submodule pin:    make agent-canon-status"
	@echo "  Docker/runtime:   make docker-check [and make docker-build-check if build behavior changed]"
	@echo "  GitHub automation: make github-workflow-check"
	@echo "  Experiment:       make experiment-check"
	@echo "  Full confidence:  make ci"

# template fresh clone acceptance
fresh-clone-check:
	bash tools/ci/check_fresh_clone.sh

# higher-level template acceptance
template-check: fresh-clone-check

# Agent workflow targets
# clone-time repository bootstrap
start-repository:
	bash scripts/start_repository.sh $(ARGS)

# machine-driven task start
task-start:
	$(PYTHON) $(AGENT_TOOLS)/task_start.py $(ARGS)

# machine-driven document start
doc-start:
	$(PYTHON) $(AGENT_TOOLS)/doc_start.py $(ARGS)

# machine-driven task close gate
task-close:
	$(PYTHON) $(AGENT_TOOLS)/task_close.py $(ARGS)

# machine-driven agent behavior evaluation
agent-evaluate:
	$(PYTHON) $(AGENT_TOOLS)/evaluate_agent_run.py $(ARGS)

# machine-driven repo-wide dependency review
dependency-review:
	bash $(AGENT_TOOLS)/run_repo_dependency_review.sh $(ARGS)

# strict dependency review for both template root views and AgentCanon source
dependency-review-surfaces:
	bash $(AGENT_TOOLS)/run_repo_dependency_review.sh --fail-missing $(ARGS)
	bash $(AGENT_TOOLS)/run_repo_dependency_review.sh --root vendor/agent-canon --fail-missing $(ARGS)

# integrated file-by-file review backlog scan
review-backlog-scan:
	bash $(AGENT_TOOLS)/review_backlog_scan.sh $(ARGS)

# machine-driven repo-wide review closeout gate
repo-wide-review-check:
	@mkdir -p "$(REPO_WIDE_REVIEW_REPORT_DIR)"
	@test -f reports/agents/.active_run
	@printf 'REPO_WIDE_REVIEW_CHECK_REPORT_DIR=%s\n' "$(REPO_WIDE_REVIEW_REPORT_DIR)" | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/report_contract.txt"
	@printf 'ACTIVE_RUN=%s\n' "$$(cat reports/agents/.active_run)" | tee -a "$(REPO_WIDE_REVIEW_REPORT_DIR)/report_contract.txt"
	bash -o pipefail -c '$(MAKE) agent-canon-ensure-latest 2>&1 | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/agent-canon-ensure-latest.txt"'
	bash -o pipefail -c '$(AGENT_CANON_SYNC) check 2>&1 | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/agent-canon-sync-check.txt"'
	bash -o pipefail -c '$(PYTHON) $(AGENT_TOOLS)/check_agent_runtime_alignment.py 2>&1 | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/agent-runtime-alignment.txt"'
	bash $(AGENT_TOOLS)/review_backlog_scan.sh --report-dir "$(REPO_WIDE_REVIEW_REPORT_DIR)/review-backlog" --check inventory --check stale --check dependency-review --check semantic-index
	$(MAKE) semantic-index-stale-check REPO_WIDE_REVIEW_REPORT_DIR="$(REPO_WIDE_REVIEW_REPORT_DIR)"
	$(MAKE) docker-check
	$(MAKE) github-workflow-check
	bash -o pipefail -c '$(PYTHON) $(AGENT_TOOLS)/check_convention_compliance.py 2>&1 | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/convention-compliance.txt"'

semantic-index-stale-check:
	@mkdir -p "$(REPO_WIDE_REVIEW_REPORT_DIR)"
	@printf '%s\n' "$(REPO_WIDE_REVIEW_QUERY)" > "$(REPO_WIDE_REVIEW_REPORT_DIR)/semantic-index-query.txt"
	bash -o pipefail -c '\
		if [ -f vendor/agent-canon/rust/agent-canon/Cargo.toml ]; then \
			agent_canon_cmd="cargo run --quiet --manifest-path vendor/agent-canon/rust/agent-canon/Cargo.toml --"; \
		else \
			agent_canon_cmd="agent-canon"; \
		fi; \
		$$agent_canon_cmd semantic-index build --root . 2>&1 | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/semantic-index-build.txt"; \
		$$agent_canon_cmd semantic-index search --root . --query-file "$(REPO_WIDE_REVIEW_REPORT_DIR)/semantic-index-query.txt" --top-k 5 --format jsonl 2>&1 | tee "$(REPO_WIDE_REVIEW_REPORT_DIR)/semantic-index-stale-check.jsonl"'

# machine-driven intermediate waterfall gate check
waterfall-gate-check:
	$(PYTHON) $(AGENT_TOOLS)/waterfall_gate_check.py $(ARGS)

# machine-driven user preference note append
user-preference-log:
	$(PYTHON) $(AGENT_TOOLS)/log_user_preference.py $(ARGS)

# Documentation and generated artifacts
# repo-wide Markdown lint / link checks
docs-check:
	tools/bin/agent-canon docs check

# remove generated, ignored artifacts that make the template workspace noisy
clean-generated:
	git clean -Xdf \
		.pytest_cache \
		.ruff_cache \
		build \
		logs \
		reports \
		tests/logs \
		.devcontainer/docker-compose.generated.yml

# GitHub and agent-runtime targets
# GitHub Actions / PR template convention checks
github-workflow-check:
	$(PYTHON) $(CI_TOOLS)/check_github_workflows.py

# agent runtime / skill drift checks
agent-checks:
	$(MAKE) agent-surface-checks

agent-surface-checks:
	bash tools/ci/check_agent_canon_latest.sh
	$(AGENT_CANON_SYNC) check
	$(PYTHON) $(AGENT_TOOLS)/check_agent_runtime_alignment.py
	$(PYTHON) $(AGENT_TOOLS)/smoke_test_research_perspective_pack.py

# AgentCanon sync/update targets
# read-only gate for upstream agent-canon freshness
agent-canon-latest-check:
	bash tools/ci/check_agent_canon_latest.sh

# shared surface drift only
agent-canon-check:
	$(AGENT_CANON_SYNC) check

# root shared surface を vendor 正本へ再リンク
agent-canon-links:
	$(AGENT_CANON_SYNC) link-root

# submodule pin / legacy tree 設定を確認
agent-canon-status:
	$(AGENT_CANON_SYNC) status

# upstream agent-canon を task 開始時に取り込む
agent-canon-ensure-latest agent-canon-latest agent-canon-update:
	$(AGENT_CANON_UPDATE) latest $(ARGS)

agent-canon-rebuild-tools:
	$(AGENT_CANON_UPDATE) rebuild-tools

agent-canon-update-plan:
	$(AGENT_CANON_UPDATE) plan $(ARGS)

agent-canon-merge-main:
	$(AGENT_CANON_UPDATE) merge-main-into-current-preserve-dirty $(ARGS)

# shared canon 専用の PR gate
agent-canon-pr-check:
	bash tools/ci/check_agent_canon_pr.sh

# Docker and runtime targets
# Dockerfile と requirements の整合
docker-check:
	bash tools/docker_dependency_validator.sh

# 現在の runtime で repo-local .venv が許可されるかを表示
python-env-status:
	$(PYTHON) $(CI_TOOLS)/python_env_policy.py

# 許可される runtime で canonical .venv を準備
python-env-prepare:
	$(PYTHON) $(CI_TOOLS)/python_env_policy.py --create

# Docker イメージ build / smoke 可否の確認
docker-build-check:
	bash docker/check_build.sh --pack $(DOCKER_DEFAULT_PACK)

# Docker socket を mount した build smoke check
docker-build-check-host-docker:
	bash docker/check_build.sh --pack $(DOCKER_HOST_PACK)

# 任意 program を canonical container で実行
docker-run:
	$(PYTHON) $(CI_TOOLS)/run_repo_program.py $(ARGS)

# devcontainer compose を canonical pack から生成
devcontainer-render:
	bash .devcontainer/generate-runtime-compose.sh

# main server host readiness
server-check:
	$(PYTHON) $(CI_TOOLS)/check_server_readiness.py --layout $(SERVER_LAYOUT)

# experiment registry validation
experiment-check:
	$(PYTHON) $(CI_TOOLS)/check_experiment_registry.py

# 既定 pack の shell を起動
docker-shell:
	$(PYTHON) $(CI_TOOLS)/run_in_repo_container.py --pack $(DOCKER_DEFAULT_PACK) --shell-session --tty

# canonical container で JupyterLab を起動
docker-jupyter:
	$(PYTHON) $(CI_TOOLS)/run_in_repo_container.py --pack $(DOCKER_DEFAULT_PACK) --keep-image --port $${JUPYTER_HOST_PORT:-8888}:8888 -- jupyter lab --ip=0.0.0.0 --port=8888 --no-browser --allow-root --ServerApp.token="$${JUPYTER_TOKEN:-project-template}"

# nested Codex を既定 pack で起動
docker-codex:
	$(PYTHON) $(CI_TOOLS)/run_codex_in_repo_container.py

# nested Codex を host Docker socket 付き pack で起動
docker-codex-host-docker:
	$(PYTHON) $(CI_TOOLS)/run_codex_in_repo_container.py --profile host-docker

# Help targets
# 開発開始の確認
dev-setup:
	@echo "Template clone is ready. Read documents/template-bootstrap.md, then run: make fresh-clone-check"

# ツール情報表示
tools-help:
	@echo "Core targets:"
	@echo "  make check-matrix        Show validation routing"
	@echo "  make ci-quick            Run quick local validation"
	@echo "  make docs-check          Run Markdown/document checks"
	@echo "  make agent-checks        Check shared agent surfaces"
	@echo "  make docker-check        Check Docker dependency boundaries"
	@echo ""
	@echo "Detailed catalog:"
	@echo "  $(PYTHON) $(AGENT_TOOLS)/tool_catalog.py --format markdown"
