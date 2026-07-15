# ptymark Runtime Dependencies

<!--
@dependency-start
contract design
responsibility Defines dependency ownership, version sources, environment boundaries, and upgrade checks.
upstream design ./ptymark-design.md renderer and terminal-safety architecture
upstream design ./ptymark-installer.md managed renderer installation contract
upstream environment ../renderers/managed-bundle.env managed runtime and package versions
upstream environment ../docker/ptymark-versions.env canonical validation image inputs
downstream tool ../scripts/check-ptymark-runtime-dependencies.mjs alignment validation
downstream workflow ../.github/workflows/ptymark-dependency-alignment.yml focused CI gate
@dependency-end
-->

Ptymark has several dependency surfaces, but they do not have the same owner or
installation semantics. This document separates the shipped Rust core, the optional
managed renderer bundle, the canonical product validation image, and repository-only
automation so that one environment is not mistaken for another.

## Dependency classes

| Class | Required for | Version or package owner |
| --- | --- | --- |
| Rust core | Building from source and producing native release executables | `rust-toolchain.toml`, with the compatibility floor mirrored in `Cargo.toml` |
| Managed renderer bundle | Default Mermaid and math rendering after an explicit install | `renderers/managed-bundle.env`, `renderers/package.json`, and `renderers/package-lock.json` |
| Canonical product validation image | Reproducible local and GitHub Actions product checks | `docker/ptymark-versions.env`, `docker/ptymark.Dockerfile`, and `docker/ptymark-compose.yaml` |
| System renderer tools | Docker smoke tests for Chromium, Chafa, Lua, and ShellCheck | the Debian packages installed by `docker/ptymark.Dockerfile` |
| Repository automation | AgentCanon synchronization, Python tooling, notebooks, and generic repository checks | the root `docker/Dockerfile`, `docker/requirements.txt`, and shared AgentCanon surfaces |

The repository automation environment is not a ptymark runtime distribution. Its
CUDA, Python, Jupyter, analysis, and agent dependencies are not required to run the
native ptymark core or a packaged release.

## User-runtime boundary

The native Rust core, terminal-safety gate, semantic detector, source fallback, and
preview fallback do not require Node.js, npm, Chromium, Mermaid, MathJax, or Chafa.
Those components are optional renderer capabilities selected during explicit
installation or configuration.

Normal rendering never performs package installation or a network request. Managed
bundle installation may download pinned artifacts only through the explicit installer
flow described in [ptymark-installer.md](./ptymark-installer.md).

## Version sources of truth

### Rust

`rust-toolchain.toml` owns the exact compiler used for development, CI, and releases.
`Cargo.toml` mirrors that value as `package.rust-version`. Docker build arguments and
workflow setup commands are duplicates required by their execution systems and must
match the toolchain file.

### Node.js and renderer packages

`renderers/managed-bundle.env` owns the managed bundle protocol version, Node.js
runtime version, and direct Mermaid, MathJax, font, and Puppeteer versions.

`renderers/package.json` declares the direct npm packages and the supported Node major
range. `renderers/package-lock.json` owns the exact transitive graph consumed by
`npm ci`. Both files must agree with the managed-bundle environment.

`docker/ptymark-versions.env` contains product-image inputs only: the Node base image,
Rust version, and local image tag. Renderer package versions must not be duplicated in
that Docker environment file.

### Browser and presenter tools

The canonical image sets `PUPPETEER_SKIP_DOWNLOAD=true` and uses
`/usr/bin/chromium`, so Docker validation does not add Puppeteer's private browser to
the image. Chromium, Chafa, Lua, and ShellCheck come from the selected Debian base
image repositories. Their exact package builds are observed by image smoke tests
rather than copied into the managed renderer version manifest.

## Alignment check

Run the focused, network-free contract:

```bash
make ptymark-runtime-dependencies
```

It verifies:

- Rust agreement across the toolchain, Cargo metadata, Docker defaults, and CI/release workflows;
- Node.js agreement across the managed bundle, npm engine range, Docker base image, and CI;
- direct renderer dependency agreement between the managed manifest, `package.json`, and lockfile root;
- required canonical-image system packages and Puppeteer browser policy;
- removal of renderer package versions from the Docker-only environment file;
- wiring from `make ptymark-check` and the focused GitHub Actions workflow.

The full product gate remains:

```bash
make ptymark-check
```

## Upgrade sequence

1. Change one dependency class at a time.
2. For Rust, update `rust-toolchain.toml`, `Cargo.toml`, the Docker input/defaults, and explicit CI/release setup pins together.
3. For Node.js, update `PTYMARK_MANAGED_NODE_VERSION`, the npm engine range and lockfile root metadata, the Docker Node image, and Node setup pins together.
4. For Mermaid, MathJax, its font package, or Puppeteer, update `renderers/managed-bundle.env`, `renderers/package.json`, and regenerate `renderers/package-lock.json` with npm lockfile tooling, then run `npm ci` before the smoke tests.
5. For Chromium, Chafa, Lua, or ShellCheck, update the canonical Dockerfile and retain the command/version smoke checks.
6. Run `make ptymark-runtime-dependencies`, then `make ptymark-check`, and use the current PR head's GitHub Actions result as merge evidence.

Dependency discovery, executable compatibility reporting, and managed-bundle lifecycle
remain owned by issue #16. This contract only organizes build, validation, and locked
package inputs; it does not change rendering or installation behavior.
