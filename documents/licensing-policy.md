# Licensing Policy

<!--
@dependency-start
contract policy
responsibility Defines ptymark, template, AgentCanon, Rust dependency, and third-party renderer licensing boundaries.
upstream design ../README.md repository ownership and distribution overview
upstream design ../LICENSE repository license text
upstream design ../vendor/agent-canon/documents/agent-canon-licensing-policy.md AgentCanon licensing boundary
upstream design ./dependencies.md Rust and renderer dependency policy
upstream environment ../Cargo.lock pins Rust crates
upstream environment ../renderers/package-lock.json pins renderer packages
downstream implementation ../Cargo.toml publishes Rust package license metadata
downstream implementation ../scripts/package-ptymark-release.sh builds release archives
@dependency-end
-->

Root project-owned content, including the ptymark Rust crate, WezTerm plugin, scripts, tests,
Docker definitions, renderer wrappers, configuration examples, and project documents, is licensed
under Apache License 2.0.

## Repository boundary

- Root `LICENSE` applies to template-owned and ptymark project-owned content.
- `Cargo.toml` keeps `license = "Apache-2.0"` while the root license is unchanged.
- `vendor/agent-canon/LICENSE` applies to AgentCanon-owned runtime, workflow, skill, tool, MCP,
  and documentation surfaces.
- Root symlink views into `vendor/agent-canon/` retain the AgentCanon license.
- Do not edit `vendor/agent-canon/LICENSE` through a root view.
- Third-party skills/assets under `vendor/agent-canon/vendor/` retain their upstream license, URL,
  and revision metadata.

## Rust dependencies

The plain binary links third-party Rust crates. They remain under their upstream licenses and are
not relicensed by the root project license.

Initial direct crates:

- `serde`: typed configuration and provenance data model
- `toml`: strict configuration parsing and effective-config serialization

`Cargo.lock` is the exact inventory input. Before release, generate a machine-readable license
report for all direct and transitive crates, review unknown/non-permissive entries, and include any
required notices with the binary archive or release notes.

## Existing rendering engines

ptymark delegates layout/typesetting to existing engines and does not relicense them.

- Mermaid, Mermaid CLI, layout plugins, icon/font packages, and transitive npm packages retain
  their upstream licenses.
- MathJax and the selected MathJax New Computer Modern font package retain their upstream
  licenses and notices.
- KaTeX and its fonts/assets retain their upstream licenses.
- Puppeteer and Chromium retain their upstream/distribution licenses.
- Typst CLI and its dependencies retain their upstream licenses.
- Noto fonts, Lua, Node.js, and Debian system packages retain their distribution licenses.

`renderers/package-lock.json` is the exact JavaScript inventory input. The canonical Docker image
is a development/test artifact and is not the normal plain-binary release.

## Renderer bundle boundary

If a future self-contained renderer bundle includes an engine, browser, font, JavaScript, CSS, or
other asset, it is distributed separately from the plain Rust archive and must provide:

- complete third-party inventory from the exact lock/package/image resolution
- version/revision and integrity digest
- license text and required notices
- source-offer or redistribution obligations where applicable
- font and icon attribution
- browser and npm security-update cadence
- explicit install location and uninstall procedure
- clear statement that a renderer bundle license does not change the root Rust code license

No renderer dependency is downloaded implicitly during a normal terminal session.

## Configuration and user-defined engines

A user configuration file is user content. ptymark does not claim ownership of it. A configured
external executable, its output, fonts, and assets retain their own licenses. Explicit project
trust authorizes execution but does not imply license compatibility.

When publishing a reusable custom engine definition, its documentation must state:

- engine program/source location
- artifact and runtime dependency licenses
- whether output embeds fonts/icons/assets
- redistribution requirements

## Generated artifacts

Mermaid SVG/PNG/PDF, MathJax SVG, KaTeX HTML/MathML, Typst SVG/PDF, and terminal images are
generated artifacts. Before redistributing them, verify engine, font, icon, and embedded asset
attribution requirements. Generated output must not be assumed to inherit Apache-2.0 automatically.

Source fallback and terminal transcripts remain user/process content; ptymark diagnostics must not
copy source into release artifacts by default.

## Release checklist

A release-changing change updates and verifies:

- `LICENSE` when the project license changes
- `Cargo.toml` package license metadata
- `Cargo.lock` crate inventory and notices
- `renderers/package-lock.json` npm inventory and notices
- Docker/apt/browser/font inventory when distributing the image or bundle
- README license/distribution text
- `documents/dependencies.md`
- this policy
- release archive/bundle layout and workflow
- generated-artifact attribution implications

Do not infer that the root project license applies to AgentCanon, third-party crates/renderers,
vendored skills, fonts, browser assets, operating-system packages, user configuration, or generated
artifacts.
