<!--
@dependency-start
contract design
responsibility Provides the task-oriented documentation map for product-owned Ptymark contracts.
upstream design ../README.md product entrypoint
downstream design ./ptymark-design.md architecture contract
downstream design ./ptymark-installer.md installation contract
downstream design ../verification/README.md verification policy
@dependency-end
-->

# Documentation map

## Start by task

| Goal | Start here | Continue with |
| --- | --- | --- |
| Build and install from source | [Root README](../README.md) | [Installer design](./ptymark-installer.md) |
| Verify or troubleshoot an installation | [Troubleshooting](./troubleshooting.md) | [Verification catalog](../verification/README.md) |
| Understand terminal and rendering safety | [Ptymark design](./ptymark-design.md) | [Interactive sessions](./interactive-session.md) |
| Render structured mathematics | [OpenMath input](./openmath.md) | [Runnable example](../examples/openmath.md) |
| Filter non-interactive command output | [Filtered command execution](./filtered-command.md) | [Architecture](./ptymark-design.md) |
| Review shell coexistence | [Shell compatibility](./shell-plugin-compatibility.md) | [Verification catalog](../verification/README.md) |
| Change dependencies | [Runtime dependencies](./ptymark-runtime-dependencies.md) | [Canonical Docker environment](../docker/README.md) |
| Prepare a release | [Release contract](./release.md) | [Release metadata check](../scripts/check-release-metadata.py) |
| Plan Alpha.5 or Beta work | [Alpha.5/Beta roadmap](./roadmap-alpha5-beta.md) | [Alpha.5 tracker](https://github.com/iwashita-nozomu/ptymark/issues/147) · [Beta.1 tracker](https://github.com/iwashita-nozomu/ptymark/issues/148) |

## Document ownership

All documents below are product-owned contracts maintained in this repository.

### Product contracts

- [`ptymark-design.md`](./ptymark-design.md): architecture, terminal invariants, rendering boundaries, and extension rules.
- [`ptymark-installer.md`](./ptymark-installer.md): source installation, managed renderer isolation, and state ownership.
- [`interactive-session.md`](./interactive-session.md): Unix PTY and Windows ConPTY behavior.
- [`filtered-command.md`](./filtered-command.md): pipe-oriented child execution.
- [`openmath.md`](./openmath.md): bounded OpenMath parsing and conversion.
- [`troubleshooting.md`](./troubleshooting.md): diagnostics, recovery, and support reports.
- [`shell-plugin-compatibility.md`](./shell-plugin-compatibility.md): reviewed shell/plugin coexistence.
- [`ptymark-runtime-dependencies.md`](./ptymark-runtime-dependencies.md): Rust and managed-renderer dependency ownership.
- [`release.md`](./release.md): source-only publication and rollback.
- [`alpha4-design.md`](./alpha4-design.md): Alpha.4 configuration and internal-policy separation.
- [`roadmap-alpha5-beta.md`](./roadmap-alpha5-beta.md): bounded Alpha.5 and Beta release trains, dependencies, and exit criteria; implementation is tracked by [#147](https://github.com/iwashita-nozomu/ptymark/issues/147) and [#148](https://github.com/iwashita-nozomu/ptymark/issues/148).

All documents in this directory are owned by Ptymark. Shared repository-template or external agent-runtime policy is not vendored into the product repository.
