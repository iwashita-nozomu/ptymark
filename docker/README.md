# Ptymark development container

The canonical product container is intentionally limited to the Rust core, managed renderers, shell tooling, Chromium, and Lua used by the WezTerm smoke test.

```bash
make ptymark-build
make ptymark-check
make ptymark-dev
make ptymark-clean
```

Files:

- `ptymark.Dockerfile`: pinned validation image;
- `ptymark-compose.yaml`: local and CI compose entrypoint;
- `ptymark-versions.env`: toolchain image/version ownership.

The container is verification infrastructure, not a release artifact. GitHub releases remain source-only.
