# Licensing Policy
<!--
@dependency-start
contract policy
responsibility Documents template and derived repository licensing ownership.
upstream design ../README.md repository ownership overview
upstream design ../LICENSE repository license text
upstream design ../vendor/agent-canon/documents/agent-canon-licensing-policy.md AgentCanon licensing boundary
downstream implementation ../pyproject.toml publishes Python package license metadata
@dependency-end
-->

This template repository is licensed under Apache License 2.0 unless a derived
repository deliberately replaces the root `LICENSE` and package metadata.

The license boundary follows repository ownership:

- Root `LICENSE` is the license for template-owned and project-owned repository
  content.
- `vendor/agent-canon/LICENSE` is the license for AgentCanon-owned shared
  runtime, workflow, skill, tool, MCP, and documentation surfaces.
- Root symlink views into `vendor/agent-canon/` keep the AgentCanon license.
- Derived repositories may choose a different project license, but they must not
  edit `vendor/agent-canon/LICENSE` through a root view.
- Third-party skills or reusable assets under `vendor/agent-canon/vendor/` must
  keep upstream URL, revision, and license metadata before they are enabled.

When a derived repository changes its project license, update these surfaces in
the same change:

- `LICENSE`
- `pyproject.toml` package license metadata, if the repository publishes Python
  packages
- README license text
- project-specific source headers, if the project uses source headers
- any release or distribution packaging metadata

Do not infer that the parent repository license applies to upstream AgentCanon
or third-party vendored skills. Those surfaces carry their own license metadata.
