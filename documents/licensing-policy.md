# Licensing Policy

<!--
@dependency-start
contract policy
responsibility Documents Ptymark source and third-party licensing ownership.
upstream design ../README.md product ownership overview
upstream design ../LICENSE repository license text
downstream implementation ../Cargo.toml package license metadata
@dependency-end
-->

Ptymark source code and product-owned repository content are licensed under the
Apache License 2.0, as declared by the root `LICENSE` and `Cargo.toml`.

Third-party Rust and managed-renderer dependencies retain their own licenses.
Their inclusion in a source checkout or isolated managed bundle does not change
those upstream terms. Dependency upgrades must keep license compatibility under
review together with security, MSRV, source-only distribution, and lockfile
changes.

Local developer tools, operating-system packages, browsers, and container images
used for verification are not redistributed as Ptymark release assets. GitHub
releases remain source-only and contain no project-uploaded executables.

When the project license or packaging metadata changes, update the following in
the same reviewed change:

- root `LICENSE`;
- `Cargo.toml` package license metadata;
- README and release documentation;
- downstream package metadata, when an independently maintained package exists.
