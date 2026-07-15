# Dependency Layers

<!--
@dependency-start
contract design
responsibility Defines the boundary between shipped product, repository runtime, and verification-only dependencies.
upstream design ./ptymark-runtime-dependencies.md shipped product dependency ownership
upstream environment ../Cargo.toml native product dependencies
upstream environment ../renderers/managed-bundle.env managed renderer dependency versions
upstream environment ../docker/requirements-runtime.txt repository Python runtime dependencies
upstream environment ../docker/requirements-dev.txt verification-only Python dependencies
downstream tool ../scripts/check-python-dependency-layers.py validates the Python layer contract
downstream workflow ../.github/workflows/python-dependency-layers.yml enforces the contract in CI
@dependency-end
-->

This repository has three dependency layers. A dependency belongs to the narrowest layer
that requires it. Adding a package to a broader layer merely because CI already has it is
not allowed.

## Layer model

| Layer | Required for | Canonical sources | Installed or exercised by |
| --- | --- | --- | --- |
| Shipped product | Building and running the native `ptymark` core, or installing the optional managed renderer bundle | `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `renderers/managed-bundle.env`, `renderers/package.json`, `renderers/package-lock.json` | product build, installer, product CI, release validation |
| Repository Python runtime | Running the canonical numerical and notebook workload environment | `docker/requirements-runtime.txt` | `docker/install_python_dependencies.sh --profile runtime` |
| Verification and development | Tests, static analysis, dependency inspection, documentation formatting, security checks, and profiling | `docker/requirements-dev.txt`, plus the package-local `verification` extra in `pyproject.toml` | `docker/install_python_dependencies.sh --profile verification`, repository CI, devcontainer setup |

The Python environment is a repository workspace facility. It is not a runtime
requirement of the shipped Rust executable. The complete product dependency contract
remains in [ptymark Product Dependencies](./ptymark-runtime-dependencies.md).

## Python manifests

### Runtime leaf

`docker/requirements-runtime.txt` contains packages needed for the canonical workload
runtime, including the numerical stack, notebook runtime, and runtime data validation.
It must not contain test runners, linters, documentation formatters, dependency audit
tools, or type stubs.

Use it for a minimal workspace installation:

```bash
bash docker/install_python_dependencies.sh "$PWD" --profile runtime
```

### Verification leaf

`docker/requirements-dev.txt` contains packages used only to establish confidence in a
change. This includes pytest, type and lint tools, documentation tooling, dependency
inspection, security metadata validation, profiling helpers, and type stubs.

The filename intentionally contains `dev`: package update automation can classify the
file as a development dependency source, while the repository-facing name remains
"verification" in documentation and CI.

### Compatibility aggregate

`docker/requirements.txt` is generated from the two canonical leaf files. It exists for
shared tooling and callers that still expect one flat requirements file; it is not an
independent source of truth.

Regenerate and validate it with:

```bash
python3 scripts/check-python-dependency-layers.py --write
python3 scripts/check-python-dependency-layers.py
```

The checker rejects stale generated content, duplicate declarations inside a layer,
packages present in both layers, installer/profile drift, incomplete CI cache inputs,
and dependency-update routing drift.

## Installer profiles

`docker/install_python_dependencies.sh` supports two explicit profiles:

- `runtime` installs only `docker/requirements-runtime.txt`;
- `verification` installs the generated aggregate, which is runtime plus verification.

The default remains `verification` so existing devcontainer, runtime-pack, and local
development entrypoints retain their previous capabilities. CI always selects
`--profile verification` explicitly. Marker hashes are profile-specific, so a prior
runtime-only installation is never mistaken for a complete verification environment.

## Package metadata

The Python package currently has no direct runtime dependencies in
`project.dependencies`; the numerical workspace stack is a container/runtime concern,
not an import requirement of the packaged template module.

`pyproject.toml` exposes a `verification` optional dependency group for package-local
checks. The historical `dev` group is retained as an exact compatibility alias. The
layer checker requires both lists to remain identical and requires every package in
those lists to be owned by `docker/requirements-dev.txt`.

## CI and update automation

Repository CI caches all three requirements files and installs the verification
profile. A focused `Python Dependency Layers` workflow validates the dependency
contract without downloading the heavy runtime stack.

Dependabot uses separate groups for:

- Python project runtime and project verification dependencies;
- container runtime and container verification dependencies.

The root Python updater excludes `docker/**`, preventing the same container dependency
from being proposed once through the repository root and again through `/docker`.
Container package groups are explicit so updates modify the correct leaf and generated
aggregate together.

## Upgrade procedure

1. Decide whether the dependency is shipped product, repository runtime, or
   verification-only.
2. Edit only the canonical source for that layer.
3. For Python changes, regenerate `docker/requirements.txt`.
4. Run the focused contract:

   ```bash
   python3 scripts/check-python-dependency-layers.py
   ```

5. Run the relevant runtime or verification checks. Dependency changes that affect the
   shipped product must also pass the product-specific checks described in
   [ptymark Product Dependencies](./ptymark-runtime-dependencies.md).
6. Use the current pull-request head's CI results as merge evidence.

A verification package must not be promoted into the runtime layer to fix a missing CI
installation. Fix the CI profile or verification manifest instead. Likewise, a package
used by normal workload execution must not be hidden in the verification layer merely
because tests happen to install it.
