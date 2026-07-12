# ptymark installer design

## 1. Goal

The installer makes one installation-time decision and leaves normal rendering free of package
installation or repeated engine discovery.

```text
user-installed programs
    -> installation-time resolver
    -> absolute executable paths
    -> user configuration
    -> immutable installation snapshot
    -> normal ptymark session
```

The initial installer does not invoke npm, Homebrew, apt, curl, a browser downloader, or another
package manager. It discovers programs the user already installed. Missing optional programs select
the built-in preview renderer rather than making the core unusable.

## 2. User flow

Install from a clone:

```bash
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
bash scripts/install.sh
```

The script performs two explicit stages:

1. `cargo install --locked --force --path REPOSITORY`;
2. `ptymark install resolve`.

The second stage resolves the known engine slots and writes:

```text
~/.config/ptymark/config.toml
~/.local/state/ptymark/install.toml
```

`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `PTYMARK_CONFIG`, and `PTYMARK_INSTALL_STATE` override the
locations. `--config` and `--state` have highest precedence for the installer command.

Inspect the result:

```bash
ptymark install status
ptymark config show
ptymark engine check
```

## 3. Initial resolution

When no configuration exists, `keep` behaves as `auto` for each slot.

```text
Mermaid
  find mmdc + find chafa -> mermaid-cli with absolute paths
  otherwise             -> built-in preview

Math
  find tex2svg + find chafa -> mathjax-cli with absolute paths
  otherwise                -> built-in preview
```

A detected layout engine is not activated without a presenter. This prevents SVG from being written
directly to terminal output.

The generated configuration stores canonical absolute executable paths. Normal rendering therefore
uses the installation result rather than repeating `PATH` selection for each block.

## 4. Idempotence and replacement

Re-running the installer preserves an existing valid configuration by default. `keep` means:

- preserve built-in `preview` or `source` choices;
- preserve an external backend and revalidate its configured executable;
- preserve detection, rendering, and cache settings not owned by the requested replacement.

Re-probe all known slots:

```bash
bash scripts/install.sh --reprobe
# equivalent engine choices:
ptymark install resolve \
  --mermaid auto \
  --math auto \
  --presenter auto
```

Replace only Mermaid while keeping every other setting:

```bash
ptymark install resolve \
  --mermaid /opt/homebrew/bin/mmdc
```

Switch Mermaid back to exact source:

```bash
ptymark install resolve --mermaid source
```

Replace the presenter:

```bash
ptymark install resolve \
  --presenter /usr/local/bin/chafa
```

Reset all project-owned configuration before resolving:

```bash
ptymark install resolve --reset
```

Preview a complete plan without writing files:

```bash
ptymark install resolve --dry-run
```

## 5. Command contract

```text
ptymark install resolve [OPTIONS]
ptymark install status [--state PATH]
```

Resolution options:

```text
--config PATH
--state PATH
--mermaid keep|auto|preview|source|EXECUTABLE
--math keep|auto|preview|source|EXECUTABLE
--presenter keep|auto|EXECUTABLE
--reset
--dry-run
```

An executable is either:

1. an absolute path; or
2. a bare name resolved from the installer process `PATH`.

Relative paths containing directory components are rejected by configuration validation. GUI terminal
sessions should normally use the absolute paths written by the installer.

## 6. Stored installation snapshot

The state file is diagnostic evidence, not mutable runtime policy.

```toml
schema_version = 1
ptymark_version = "0.1.0-alpha.1"
config_path = "/home/user/.config/ptymark/config.toml"

[[components]]
role = "mermaid"
backend = "mermaid-cli"
active = true
origin = "path-search"
requested_path = "mmdc"
resolved_path = "/home/user/.local/bin/mmdc"
```

Each component records:

- logical role;
- selected backend;
- whether it is active;
- resolution origin;
- requested path;
- resolved absolute path;
- optional fallback note.

`ptymark install status` rereads the snapshot and checks whether each resolved executable still
exists and is executable. It does not silently select a replacement. Replacement is always an
explicit `install resolve` operation.

## 7. Extension boundary

The installer orchestration depends on one small interface:

```rust
pub trait ProgramResolver: Send + Sync {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError>;
}
```

The initial `PathProgramResolver` supports absolute paths and conventional `PATH` lookup. A future
resolver may support a managed renderer bundle, a platform package directory, or another verified
source without changing:

- `InstallRequest`;
- `InstallPlan`;
- configuration serialization;
- installation-state serialization;
- render decision and engine handoff;
- terminal stream processing.

The installer still has concrete Mermaid, math, and presenter slots. It is not a dynamic plugin
registry. A new slot is added only with a concrete engine protocol, installation route, fallback,
license owner, and end-to-end test.

## 8. Failure policy

| Situation | Automatic request | Explicit or preserved external request |
| --- | --- | --- |
| layout engine missing | built-in preview | installation error |
| presenter missing | built-in preview | installation error |
| invalid existing config | installation error | installation error |
| state file missing | `install status` error | rerun `install resolve` |
| resolved executable removed later | status reports `missing` | explicit re-resolution required |
| write failure | no successful install result | fix permissions/path and rerun |

Configuration and state files are written through same-directory temporary files and renamed only
after the content has been flushed.

## 9. Security boundary

- no shell command strings are stored in configuration;
- no arbitrary argument templates are accepted;
- no package manager runs during rendering;
- no network request runs during resolution;
- external paths become explicit absolute paths in the generated configuration;
- project-local configuration is not discovered automatically;
- render failures continue to restore exact source unless strict mode is enabled.

## 10. Verification

Rust contract tests cover:

- first-install automatic resolution;
- missing-presenter fallback;
- explicit missing-engine failure;
- one-slot replacement with unrelated settings preserved;
- reset behavior;
- installation-state round trip.

The canonical Docker job also runs `tests/install_smoke.sh`, which exercises the shell installer,
writes a real configuration and state file, renders through protocol-faithful fake executables, and
replaces one engine without resetting the other slot.
