# ptymark Product Dependencies

<!--
@dependency-start
contract design
responsibility Defines product dependency ownership, version sources, and upgrade checks.
upstream design ./ptymark-design.md renderer and terminal-safety architecture
upstream design ./ptymark-installer.md managed renderer installation contract
upstream environment ../renderers/managed-bundle.env managed runtime and package versions
downstream tool ../scripts/check-ptymark-runtime-dependencies.mjs alignment validation
downstream workflow ../.github/workflows/ptymark-dependency-alignment.yml focused CI gate
@dependency-end
-->

This contract covers dependencies that affect the ptymark executable built from source or the
optional managed renderer bundle. It deliberately does not treat the contents of a
development or CI container as product dependencies.

## Dependency classes

| Class | Required for | Version or package owner |
| --- | --- | --- |
| Rust core | Building from source and running native product verification | `rust-toolchain.toml`, with the compatibility floor mirrored in `Cargo.toml` |
| Managed renderer bundle | Default Mermaid and math rendering after an explicit install | `renderers/managed-bundle.env`, `renderers/package.json`, and `renderers/package-lock.json` |

The native Rust core, terminal-safety gate, semantic detector, source fallback, and
preview fallback do not require Node.js, npm, Chromium, Mermaid, MathJax, or Chafa.
The JavaScript packages are needed only when the managed renderer bundle is selected.

Normal rendering never installs packages or performs a network request. Managed bundle
installation may download pinned artifacts only through the explicit installer flow
described in [ptymark-installer.md](./ptymark-installer.md).

## Intentionally outside this contract

The following are execution-environment details, not product dependency sources:

- Debian packages and environment variables inside `docker/ptymark.Dockerfile`;
- Docker base-image, Compose, and local image-tag settings;
- tools used only by container smoke tests, such as ShellCheck and Lua;
- the generic AgentCanon/Python/Jupyter repository environment;
- software preinstalled on GitHub-hosted runners.

The alignment checker does not read any file under `docker/`. Those surfaces may be
changed when their own build or CI requirements change without expanding this product
dependency contract. Runtime executable discovery, browser compatibility, external
presenter selection, and managed-bundle lifecycle remain owned by issue #16.

## Sources of truth

### Rust

`rust-toolchain.toml` owns the exact compiler used for source development and product
CI. `Cargo.toml` mirrors the same value as `package.rust-version`. Explicit Rust setup
commands in the product CI workflow must match that toolchain version. The source-only
release workflow does not build executables and therefore does not install Rust.

### Managed Node.js and renderer packages

`renderers/managed-bundle.env` owns the managed bundle protocol version, private
Node.js runtime version, and direct Mermaid, MathJax, font, and Puppeteer versions.

`renderers/package.json` declares the direct npm packages and supported Node major
range. `renderers/package-lock.json` owns the exact transitive graph consumed by
`npm ci`. The manifest, lockfile root, and managed-bundle environment must agree.

## Alignment check

Run the focused, network-free contract:

```bash
make ptymark-runtime-dependencies
```

It verifies:

- Rust agreement across the toolchain, Cargo metadata, and explicit product CI pins;
- Node.js agreement across the managed bundle, npm engine range, and renderer CI;
- direct renderer dependency agreement between the managed manifest, `package.json`,
  and the lockfile root;
- wiring from the product make target and focused GitHub Actions workflow.

The full product gate also invokes the same check:

```bash
make ptymark-check
```

Running it through the canonical container does not make the container's own packages
part of this dependency contract.

## Upgrade sequence

1. Change one product dependency class at a time.
2. For Rust, update `rust-toolchain.toml`, `Cargo.toml`, and explicit product CI
   workflow setup pins together.
3. For Node.js, update `PTYMARK_MANAGED_NODE_VERSION`, the npm engine range, the
   lockfile root metadata, and renderer workflow setup pins together.
4. For Mermaid, MathJax, its font package, or Puppeteer, update
   `renderers/managed-bundle.env`, `renderers/package.json`, and regenerate
   `renderers/package-lock.json` with npm lockfile tooling.
5. Run `make ptymark-runtime-dependencies`, then the relevant product smoke tests, and
   use the current PR head's GitHub Actions result as merge evidence.

This contract organizes shipped and managed-bundle inputs only. It does not change
normal rendering, installation, fallback, or executable discovery behavior.
