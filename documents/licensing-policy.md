# Licensing Policy

<!--
@dependency-start
contract policy
responsibility Defines ptymark, template, AgentCanon, and third-party renderer licensing boundaries.
upstream design ../README.md repository ownership and distribution overview
upstream design ../LICENSE repository license text
upstream design ../vendor/agent-canon/documents/agent-canon-licensing-policy.md AgentCanon licensing boundary
upstream design ./dependencies.md external renderer dependency policy
downstream implementation ../Cargo.toml publishes Rust package license metadata
downstream implementation ../scripts/package-ptymark-release.sh builds release archives
@dependency-end
-->

Root project-owned content, including the ptymark Rust crate, WezTerm plugin, scripts, tests,
Docker definitions, and project documents, is licensed under Apache License 2.0.

## Repository boundary

- Root `LICENSE` applies to template-owned and ptymark project-owned content.
- `Cargo.toml` must keep `license = "Apache-2.0"` while the root license is unchanged.
- `vendor/agent-canon/LICENSE` applies to AgentCanon-owned runtime, workflow, skill, tool,
  MCP, and documentation surfaces.
- Root symlink views into `vendor/agent-canon/` retain the AgentCanon license.
- Do not edit `vendor/agent-canon/LICENSE` through a root view.
- Third-party skills/assets under `vendor/agent-canon/vendor/` retain their upstream license,
  URL, and revision metadata.

## Existing rendering engines

ptymark delegates layout/typesetting to existing engines and does not relicense them.

- Mermaid CLI and its transitive packages retain their upstream licenses.
- KaTeX and its fonts/assets retain their upstream licenses.
- Typst CLI and its dependencies retain their upstream licenses.
- Debian Chromium, Noto fonts, Lua, Node.js, and system packages retain their distribution
  licenses.

These tools are installed only in the development/test image and are not bundled into the
normal ptymark release archive.

If a future self-contained renderer bundle includes an engine, browser, font, JavaScript,
CSS, or other asset, that bundle must provide:

- complete third-party inventory
- version/revision
- license text and required notices
- source-offer or redistribution obligations where applicable
- separation from the plain ptymark binary archive

## Generated artifacts

Mermaid SVG, KaTeX HTML/MathML, Typst SVG/PDF, and terminal images are generated artifacts.
Before distributing generated assets, verify engine and font attribution requirements.
Generated output must not be assumed to inherit Apache-2.0 automatically.

## Release checklist

A release-changing change updates these surfaces together:

- `LICENSE` when the project license changes
- `Cargo.toml` license metadata
- README license/distribution text
- `documents/dependencies.md`
- this policy
- release archive layout and workflow
- third-party license inventory for bundled content

Do not infer that the root project license applies to AgentCanon, third-party renderers,
vendored skills, fonts, or operating-system packages.
