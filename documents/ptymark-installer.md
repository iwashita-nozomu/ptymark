# ptymark installer design

## 1. Purpose

The installer must leave the user with a usable pre-display renderer after one platform-appropriate
command while keeping normal terminal sessions free of installation, network access, and repeated
engine discovery.

```text
platform installer
    -> install native ptymark core
    -> discover user-provided renderer programs
    -> provision only missing default roles in an isolated bundle
    -> convert paths to the host-native representation
    -> call the shared Rust resolver
    -> atomically write runtime configuration and installation state
    -> normal rendering performs no installation
```

The installer may affect only ptymark-owned files and configuration. It does not alter terminal input,
termios, signal forwarding, resize forwarding, child-process behavior, or terminal control sequences.

## 2. Responsibility split

Installation is divided into three layers.

### 2.1 Shell and OS frontend

```text
scripts/installer.sh
scripts/installer.ps1
scripts/installer.cmd
```

A frontend owns only concerns that genuinely differ by host or shell:

- locating the installed core binary;
- choosing standard config, state, and data directories;
- detecting commands visible to that shell;
- installing the isolated renderer bundle;
- converting shell-specific paths to native absolute paths;
- presenting platform-specific help and errors.

It must not independently merge TOML, decide cache semantics, or implement a second engine-selection
model.

### 2.2 Shared Rust resolver

```text
ptymark install resolve
ptymark install status
```

The Rust implementation owns:

- the typed `InstallRequest`;
- engine-slot selection semantics;
- preservation of unrelated user configuration;
- cross-field validation;
- atomic config and state writes;
- installation inventory and readiness reporting.

Every frontend ends by calling this resolver. Consequently, PowerShell, cmd.exe, Git Bash, MSYS2,
Linux, macOS, and WSL produce the same semantic configuration.

### 2.3 Managed-bundle installer

```text
scripts/install-managed-bundle.sh
scripts/install-managed-bundle.ps1
```

This layer installs a pinned renderer implementation under a versioned ptymark-owned data directory.
It never changes a global npm prefix or the user's `PATH`.

## 3. Supported entrypoints

### 3.1 Linux, macOS, and WSL

```bash
bash scripts/installer.sh
```

`uname -s` selects Linux or macOS data locations. WSL reports Linux and is deliberately treated as a
Linux environment:

```text
WSL shell
    -> Linux ptymark binary
    -> Linux paths
    -> Linux renderer bundle
```

The installer does not silently mix a WSL process with Windows `.exe` renderer paths.

### 3.2 Windows PowerShell

```powershell
pwsh -File scripts/installer.ps1
```

Windows PowerShell 5.1 is also a supported frontend:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/installer.ps1
```

This is the canonical Windows-native implementation.

### 3.3 Windows cmd.exe

```bat
scripts\installer.cmd
```

The cmd file contains no installation policy. It selects `pwsh.exe` when available, falls back to
`powershell.exe`, and delegates all arguments to `installer.ps1`.

### 3.4 Git Bash, MSYS2, and Cygwin

```bash
bash scripts/installer.sh
```

The Bash frontend recognizes `MINGW*`, `MSYS*`, and `CYGWIN*`, then performs only the bridge work:

```text
POSIX-looking Bash arguments
    -> classify option as path, enum, or scalar
    -> cygpath -aw for path-valued options
    -> disable MSYS automatic argv conversion
    -> invoke installer.ps1
```

This prevents mixed path representations such as `/c/Users/...` in a configuration consumed by
`ptymark.exe`.

## 4. Path contract

Paths have an explicit owner and normalization point.

| Value | Owner | Stored form |
| --- | --- | --- |
| core binary | platform frontend | host-native absolute path |
| config path | platform frontend | host-native absolute path |
| state path | platform frontend | host-native absolute path |
| managed bundle root | platform frontend | host-native absolute path |
| selected engine path | Rust resolver input | canonical absolute path |
| runtime config engine path | Rust resolver | canonical absolute path |
| installation-state paths | Rust resolver | canonical absolute path |

Rules:

1. an explicit executable may be an absolute path or a bare command name;
2. a bare name is searched only during installation or an explicit runtime check;
3. a relative path containing directory components is rejected;
4. generated runtime configuration always stores absolute paths;
5. Git Bash/MSYS/Cygwin paths are converted before the Rust resolver sees them;
6. WSL paths remain Linux paths;
7. normal rendering never repeats installation-time candidate ranking.

## 5. One-command flow

On a new installation, the frontend performs:

```text
1. cargo install --locked --force --path REPOSITORY
2. resolve platform config/data/state destinations
3. inspect explicit options and visible system commands
4. determine whether any renderer role is missing
5. install or reuse the versioned managed bundle when allowed
6. select absolute engine and presenter paths
7. invoke `ptymark install resolve`
8. invoke `ptymark install status`
```

A valid existing configuration changes the behavior of an ordinary rerun: existing choices and all
unrelated detector, render, and cache settings are retained. `--reprobe` or `-Reprobe` asks the
frontend to inspect current commands and managed fallbacks again.

## 6. Engine slots and selection

The initial installer has three concrete slots:

```text
Mermaid layout
Math layout
Terminal presenter
```

This is intentionally not a generic plugin registry. Each slot has a known input/output protocol and a
known configuration representation.

First-install selection order:

```text
1. explicit user choice
2. compatible executable visible to the installer shell
3. existing complete ptymark-managed bundle
4. install the pinned managed bundle
5. built-in preview when managed installation is disabled
```

Ordinary rerun order:

```text
1. explicit replacement
2. preserved valid existing choice
3. first-install rules only for a reset or reprobe
```

An external layout engine is activated only when a presenter is also available. SVG is never emitted
directly to terminal stdout.

## 7. Default managed implementation

The tested default set is pinned as a unit:

| Component | Version or role |
| --- | --- |
| Mermaid | `@mermaid-js/mermaid-cli` 11.16.0 |
| TeX math | MathJax 4.1.3 |
| runtime | Node.js 24.18.0 |
| browser bridge | Puppeteer 25.2.1 |
| terminal output | ptymark ANSI/Unicode presenter |

JavaScript is not part of the core architecture contract. It is an implementation detail of the
selected default renderer bundle:

```text
Rust detector / safety / cache / routing
    -> fixed engine handoff
    -> optional managed JavaScript implementation
    -> SVG artifact
    -> terminal-safe bytes
```

The built-in preview and source routes work without Node or npm. A future native engine can replace a
slot without changing the terminal gate, detector, cache, installer state schema, or display commit.

## 8. Bundle isolation and layout

Default roots:

```text
Linux
  ${XDG_DATA_HOME:-~/.local/share}/ptymark/renderer-bundles/<bundle-id>/

macOS
  ~/Library/Application Support/ptymark/renderer-bundles/<bundle-id>/

Windows
  %LOCALAPPDATA%\ptymark\renderer-bundles\<bundle-id>\
```

The bundle identifier includes the ptymark bundle schema and selected component versions so multiple
versions can coexist.

```text
<bundle-id>/
├── bundle.toml
├── bundle.stamp
├── app/
│   ├── package.json
│   ├── package-lock.json
│   ├── node_modules/
│   └── managed/
├── runtime/
├── cache/
│   ├── npm/
│   └── puppeteer/
└── bin/
    ├── mmdc[.exe]
    ├── tex2svg[.exe]
    └── chafa[.exe]
```

The `bin` entries are copies or hard links of the native ptymark binary. The executable name selects a
fixed role. The launcher validates `bundle.toml` and invokes the configured Node runtime directly with
a fixed entrypoint. It does not create a shell command string or generated batch wrapper.

## 9. Browser selection

A browser is needed by Mermaid and the ANSI presenter. The frontend chooses in this order:

```text
1. explicit --browser / -Browser path
2. installed Chromium-compatible browser
3. Puppeteer's private compatible browser inside the bundle cache
```

Typical installed candidates include Chromium, Chrome, and Microsoft Edge. Selecting an installed
browser automatically enables the no-download path for the bundle install.

`--skip-browser-download`/`-SkipBrowserDownload` requires a usable installed browser. `--offline` and
`-Offline` prohibit both browser and package downloads.

## 10. Integrity and network boundary

Installation-time network access is explicit and limited to managed provisioning.

- an official Node archive is checked against the official `SHASUMS256.txt` entry;
- JavaScript dependencies are resolved with `npm ci` from the committed lockfile;
- npm cache and Puppeteer cache are bundle-local;
- the stamp records lockfile, launcher, runtime, and browser identity;
- normal rendering never invokes npm, downloads a browser, or performs dependency resolution.

The installer does not modify Homebrew, apt, winget, Chocolatey, a global npm prefix, or user `PATH`.

## 11. Replacement and idempotence

Replace one slot without touching other settings:

```bash
bash scripts/installer.sh --mermaid /absolute/path/to/mmdc
bash scripts/installer.sh --math source
```

```powershell
pwsh -File scripts/installer.ps1 -Mermaid 'C:\Tools\mmdc.exe'
pwsh -File scripts/installer.ps1 -Math source
```

Re-probe all known slots:

```bash
bash scripts/installer.sh --reprobe
```

```powershell
pwsh -File scripts/installer.ps1 -Reprobe
```

Force the managed set:

```text
--managed always
-Managed always
```

Disable managed provisioning:

```text
--managed never
-Managed never
```

Reset discards ptymark-owned configuration values before resolving again. A dry run prints the plan
without writing config or state.

## 12. Stored installation state

The state file is diagnostic evidence, not runtime policy.

```toml
schema_version = 1
ptymark_version = "0.1.0-alpha.1"
config_path = "C:\\Users\\name\\AppData\\Roaming\\ptymark\\config.toml"

[[components]]
role = "mermaid"
backend = "mermaid-cli"
active = true
origin = "explicit"
requested_path = "C:\\...\\mmdc.exe"
resolved_path = "C:\\...\\mmdc.exe"
```

Each component records role, backend, activation, resolution origin, requested path, resolved path, and
an optional fallback note. `install status` verifies the recorded executable; it does not silently
choose a replacement.

## 13. Extension boundaries

### Program discovery

```rust
pub trait ProgramResolver: Send + Sync {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError>;
}
```

`PathProgramResolver` supports absolute paths and platform `PATH`/`PATHEXT` conventions. A verified
package directory or signed-bundle resolver can implement the same interface without changing config
or state serialization.

### Frontend extension

A new shell frontend must:

1. map its option syntax to the canonical installer values;
2. normalize path-valued inputs to host-native absolute paths;
3. use standard host-owned config/data/state directories;
4. invoke the shared Rust resolver;
5. preserve exact exit status and diagnostics;
6. add a GitHub Actions smoke test.

It must not implement a competing TOML merge or renderer-ranking algorithm.

### Engine extension

A new engine slot requires:

- a concrete user use case;
- a fixed input/output protocol;
- bounded execution and artifact validation;
- exact-source fallback;
- install and replacement behavior;
- dependency and license ownership;
- Ubuntu, macOS, and Windows tests.

## 14. Failure policy

| Situation | Automatic install | Explicit or preserved external choice |
| --- | --- | --- |
| system layout engine missing | managed fallback | error if explicitly required |
| system presenter missing | managed fallback | error if explicitly required |
| managed install disabled | built-in preview | external choice remains required |
| offline bundle incomplete | clear install error | clear install error |
| invalid existing config | stop before child process | stop before child process |
| selected executable removed later | status reports missing | explicit rerun/replacement required |
| config/state write failure | no successful installation result | fix path/permissions and rerun |

Config and state files are written via same-directory temporary files and renamed only after content is
flushed.

## 15. Verification

The canonical GitHub Actions workflow verifies:

- Rust formatting, Clippy, and tests on Ubuntu, macOS, and Windows;
- PowerShell parser and Bash syntax checks;
- Windows `PATHEXT` executable resolution;
- the PowerShell installer with a real isolated managed bundle;
- the cmd.exe bridge;
- the Git Bash/MSYS path-conversion bridge;
- real Mermaid and MathJax rendering on Windows;
- POSIX installer contracts in the canonical Docker image;
- isolated managed-bundle rendering in Docker;
- compatibility wrappers and one-slot replacement;
- WezTerm Unix and Windows examples.

GitHub Actions is the merge gate. A frontend is not considered supported merely because its source
parses; its path conversion, configuration output, inventory, and render path must execute on the
corresponding hosted runner.
