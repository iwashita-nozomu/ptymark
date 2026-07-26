<!--
@dependency-start
contract design
responsibility Provides the task-oriented documentation map and records document ownership.
upstream design ../README.md product entrypoint
upstream design ../vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md shared ownership policy
upstream design ../vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md parent readiness policy
downstream design ./openmath.md structured math input contract
downstream design ./ptymark-design.md architecture contract
downstream design ./ptymark-installer.md installation contract
downstream design ./shell-plugin-compatibility.md coexistence evidence
downstream design ../verification/README.md verification policy
@dependency-end
-->

# Documentation map

Start from the task you are trying to complete. Product behavior belongs in the project documents linked near the top; repository ownership and shared AgentCanon policy are recorded later for maintainers.

## Start by task

| Goal | Start here | Continue with |
| --- | --- | --- |
| Build and install from source | [README: build and install](../README.md#build-and-install-from-a-source-checkout) | [Installer design](./ptymark-installer.md) |
| Verify an installation | [README: verify installation](../README.md#verify-installation) | [Troubleshooting](./troubleshooting.md) |
| Render Mermaid or TeX | [README: use `preview`](../README.md#use-preview) | [Architecture](./ptymark-design.md) |
| Render structured mathematics | [OpenMath input](./openmath.md) | [Runnable OpenMath example](../examples/openmath.md) |
| Run an interactive shell or command | [Interactive PTY and ConPTY session](./interactive-session.md) | [Terminal safety in the architecture](./ptymark-design.md#3-terminal-and-stream-invariants) |
| Filter a batch or log-producing command | [Filtered command execution](./filtered-command.md) | [README: safety and failure behavior](../README.md#safety-and-failure-behavior) |
| Recover from a problem or prepare a report | [Troubleshooting](./troubleshooting.md) | [README: diagnose and recover safely](../README.md#diagnose-and-recover-safely) |
| Configure engines and cache behavior | [README: configuration](../README.md#configuration) | [Configuration examples](../examples/README.md) |
| Add Ptymark to WezTerm | [WezTerm example](../examples/README.md#wezterm) | [Interactive session](./interactive-session.md) |
| Check shell/plugin coexistence | [Shell and rich-plugin compatibility](./shell-plugin-compatibility.md) | [Verification catalog](../verification/README.md) |
| Change the rendering architecture | [Ptymark design](./ptymark-design.md) | [Verification catalog](../verification/README.md) |
| Prepare or review a release | [Release and recovery contract](./release.md) | [Product dependencies](./ptymark-runtime-dependencies.md) |

## Recommended reading paths

### New user

1. Build and install from the root README.
2. Run the verification commands and `ptymark doctor`.
3. Try `ptymark preview` with Mermaid, TeX, or the OpenMath example.
4. Add the WezTerm launcher only after the native command works.
5. Use troubleshooting for recovery; do not infer setup steps from architecture documents.

### Contributor changing semantic rendering

1. Read the terminal and stream invariants in [`ptymark-design.md`](./ptymark-design.md).
2. Read the format-specific contract, such as [`openmath.md`](./openmath.md).
3. Keep detection, format adaptation, engine selection, presentation, cache, and display commit as separate boundaries.
4. Add unit, chunk-boundary, fallback, native-platform, and documentation evidence to the verification catalog where applicable.

### Maintainer changing installation or dependencies

1. Read [`ptymark-installer.md`](./ptymark-installer.md).
2. Identify the dependency owner in [`ptymark-runtime-dependencies.md`](./ptymark-runtime-dependencies.md) or [`dependency-layers.md`](./dependency-layers.md).
3. Follow the release and recovery contract before changing any public distribution surface.

## Project-owned product documents

- [Ptymark design](./ptymark-design.md): current pre-display architecture, terminal-safety invariants, source-format adaptation, render decisions, typed engine handoff, cache identity, extension rules, and test strategy.
- [OpenMath input](./openmath.md): explicit fence, supported OpenMath XML object model, Content Dictionary rendering, bounds, failure behavior, and non-goals.
- [Interactive PTY and ConPTY session](./interactive-session.md): native child terminal allocation, raw mode, input and resize forwarding, filtered output, exit status, and real-process evidence.
- [Filtered command execution](./filtered-command.md): pipe-based `ptymark run -- COMMAND`, stdout filtering, inherited stdin/stderr, and explicit PTY limitations.
- [Troubleshooting](./troubleshooting.md): doctor findings, recovery ordering, redaction, and safe public support reports.
- [Ptymark installer design](./ptymark-installer.md): source installation, platform frontends, renderer resolution, managed bundle isolation, replacement, and failure policy.
- [Release and recovery contract](./release.md): source-only releases, immutable tags, rollback, and requirements for any future signed binary channel.
- [Ptymark product dependencies](./ptymark-runtime-dependencies.md): shipped Rust and managed-renderer version ownership and safe upgrade sequence.
- [Dependency layers](./dependency-layers.md): workload runtime, verification-only tools, installer profiles, and update automation.
- [Shell and rich-plugin compatibility](./shell-plugin-compatibility.md): behavior profiles and reviewed Bash, Zsh, Fish, PowerShell, and Nushell integrations.
- [Verification catalog](../verification/README.md): machine-readable merge gates, commands, evidence levels, and check/artifact names.

## Examples

- [Example index](../examples/README.md): WezTerm setup, configuration examples, and semantic input samples.
- [OpenMath sample](../examples/openmath.md): standard and project-specific Content Dictionary symbols.
- [Validated TOML](../examples/ptymark.toml): representative runtime configuration.
- [External engine TOML](../examples/external-engines.toml): explicit executable selection.
- [WezTerm configuration](../examples/wezterm.lua): append-only launcher integration.

## Document ownership

`documents/` contains project-owned product contracts, template-owned active contracts, and references to AgentCanon-owned shared policy. Edit the actual owner rather than copying shared policy into a product document.

| Class | Examples | Edit source |
| --- | --- | --- |
| Project-owned product contract | architecture, OpenMath, runtime, installer, release, compatibility | root `documents/` regular file |
| Template-owned active contract | bootstrap, host, remote execution, licensing, repository audit | root `documents/` regular file |
| AgentCanon-owned shared policy source | coding conventions, review process, shared workflow policy and templates | `vendor/agent-canon/documents/` |
| Generated or run artifact | reports, experiment outputs, logs | `reports/` or `experiments/`, not `documents/` |

The root `documents/README.md` is project-owned and remains a regular file after template cloning. AgentCanon may seed an initial index, but the derived repository owns its reader flow.

## Template-owned active contracts

These files remain regular files in the derived repository:

- [Template bootstrap](./template-bootstrap.md)
- [Licensing policy](./licensing-policy.md)
- [Template GitHub remote](./template-github-remote.md)
- [Linux / WSL host requirements](./linux-wsl-host-requirements.md)
- [Server host contract](./server-host-contract.md)
- [Remote execution repository contract](./remote-execution-repo-contract.md)
- [Repository audit checklist](./repository-audit-checklist.md)

AgentCanon provides reusable contract templates under [`vendor/agent-canon/documents/templates/`](../vendor/agent-canon/documents/templates/), but the active contract for this repository belongs here.

## Shared AgentCanon policy references

Use these only when changing repository-wide policy or shared tooling:

- [Runtime profiles and check matrix](../vendor/agent-canon/documents/runtime-profiles-and-check-matrix.md)
- [Runtime profiles inventory JSON](../vendor/agent-canon/documents/runtime-profiles-and-check-matrix.json)
- [Template / AgentCanon audit resolution](../vendor/agent-canon/documents/template-agent-canon-audit-resolution.md)
- [Shared runtime surfaces](../vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md)
- [Shared runtime surface manifest](../vendor/agent-canon/documents/shared-runtime-surfaces.toml)
- [AgentCanon parent repository latest-state checklist](../vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md)
- [Codex configuration reference](../vendor/agent-canon/documents/codex-configuration-reference.md)
- [AgentCanon GitHub remote](../vendor/agent-canon/documents/agent-canon-github-remote.md)
- [Algorithm implementation boundary policy](../vendor/agent-canon/documents/algorithm-implementation-boundary.md)
- [Object-oriented design policy](../vendor/agent-canon/documents/object-oriented-design.md)
- [Python coding conventions](../vendor/agent-canon/documents/coding-conventions-python.md)
- [Project coding conventions](../vendor/agent-canon/documents/coding-conventions-project.md)
- [Result log retention and visualization](../vendor/agent-canon/documents/result-log-retention-and-visualization.md)
- [Repository-local tool imports](../vendor/agent-canon/documents/repo-local-tool-imports.md)
