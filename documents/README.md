<!--
@dependency-start
contract design
responsibility Indexes ptymark project documents while preserving template and AgentCanon ownership boundaries.
upstream design ../vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md documents ownership policy
upstream design ../vendor/agent-canon/documents/shared-runtime-surfaces.toml machine-readable ownership manifest
upstream design ../vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md runtime profile and validation routing policy
downstream design ./architecture.md ptymark pre-display renderer contract
downstream design ./renderer-architecture.md renderer, coordinator, presenter, and cache boundaries
downstream design ./configuration.md ptymark user configuration contract
downstream design ./ui-design.md ptymark terminal UI contract
downstream design ./licensing-policy.md repository license boundary
@dependency-end
-->

# documents/

`documents/`はrepo-local index、ptymark project docs、template-owned active
contractsを置くmixed documentation directoryです。AgentCanon-owned shared policyは
`vendor/agent-canon/documents/`を正本とし、このindexから参照します。

## ptymark Project Documents

| Document | Responsibility |
| --- | --- |
| [Architecture](./architecture.md) | pre-display renderer、detector、display writerの基本設計 |
| [Renderer Architecture](./renderer-architecture.md) | engine registry/selector、coordinator、presenter、独立cache、性能境界 |
| [Configuration](./configuration.md) | TOML schema、探索順序、profile、typed session policy、validation |
| [UI Design](./ui-design.md) | streaming UI、resize、theme、image backend、cache lifecycle |
| [Usage](./usage.md) | CLI、fallback、external renderer protocol、WezTerm plugin |
| [Dependencies](./dependencies.md) | Rust/Docker/Mermaid/MathJax/KaTeX/Typst pins and update policy |
| [Development Environment](./development-environment.md) | product Docker checks and retained template environment |
| [Distribution](./distribution.md) | install routes、archive layout、release Actions |
| [Licensing Policy](./licensing-policy.md) | root、AgentCanon、third-party engineのlicense boundary |

UI runtimeのlive resize、image placement、persistent cacheは
[Issue #3](https://github.com/iwashita-nozomu/ptymark/issues/3)で追跡します。renderer、性能、
設定利用例は[Issue #4](https://github.com/iwashita-nozomu/ptymark/issues/4)と
[Issue #5](https://github.com/iwashita-nozomu/ptymark/issues/5)以降で個別管理します。

## Ownership Matrix

| Class | Examples | Edit source |
| --- | --- | --- |
| AgentCanon-owned shared policy source | coding conventions, review process, workflows, shared templates, tool docs | `vendor/agent-canon/documents/` |
| Template-owned active contract | bootstrap, host requirements, server contract, remote execution, template remote, licensing boundary | root `documents/` regular files |
| Project-owned docs | ptymark architecture, UI, configuration, dependencies, distribution | root `documents/` regular files |
| Generated or run artifacts | agent reports, experiment outputs, logs, renderer smoke artifacts | `reports/`, `experiments/`, or temporary build paths |

AgentCanon-owned fileは`vendor/agent-canon/`側を編集します。root regular fileとして
project-owned documentを維持し、shared canonをrootへ複製しません。

## Canon Runtime References

- [Runtime Profiles And Check Matrix](../vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md)
- [Runtime Profiles Inventory JSON](../vendor/agent-canon/documents/runtime-profiles-and-check-matrix.json)
- [Shared Runtime Surfaces](../vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md)
- [Shared Runtime Surface Manifest](../vendor/agent-canon/documents/shared-runtime-surfaces.toml)
- [AgentCanon Parent Repository Latest-State Checklist](../vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md)
- [Codex Configuration Reference](../vendor/agent-canon/documents/codex-configuration-reference.md)
- [AgentCanon GitHub Remote](../vendor/agent-canon/documents/agent-canon-github-remote.md)

## Coding Policy References

- [Algorithm Implementation Boundary Policy](../vendor/agent-canon/documents/algorithm-implementation-boundary.md)
- [Object-Oriented Design Policy](../vendor/agent-canon/documents/object-oriented-design.md)
- [Python Coding Conventions](../vendor/agent-canon/documents/coding-conventions-python.md)
- [Project Coding Conventions](../vendor/agent-canon/documents/coding-conventions-project.md)
- [Testing Conventions](../vendor/agent-canon/documents/coding-conventions-testing.md)

## Template-Owned Active Contracts

These remain regular files in this repository:

- [Template Bootstrap](./template-bootstrap.md)
- [Licensing Policy](./licensing-policy.md)
- [Template GitHub Remote](./template-github-remote.md)
- [Linux / WSL Host Requirements](./linux-wsl-host-requirements.md)
- [Server Host Contract](./server-host-contract.md)
- [Remote Execution Repo Contract](./remote-execution-repo-contract.md)
- [Repository Audit Checklist](./repository-audit-checklist.md)

AgentCanon provides reusable templates under
[templates/](../vendor/agent-canon/documents/templates/), but active project contracts remain
root regular files.

## Tooling And Artifact References

- [Result Log Retention And Visualization](../vendor/agent-canon/documents/result-log-retention-and-visualization.md)
- [Repo-Local Tool Imports](../vendor/agent-canon/documents/repo-local-tool-imports.md)
- `ptymark.mk`: product checks layered over the retained template `Makefile`
- `.github/workflows/ptymark-ci.yml`: product CI and benchmark evidence
- `.github/workflows/ptymark-release.yml`: native archive/release flow
