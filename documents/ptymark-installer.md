# ptymark installer design

## 1. Goal

The setup path must produce a usable renderer with one platform command while keeping normal terminal
sessions free of package installation, network access, and repeated engine discovery.

```text
source checkout
    -> install native ptymark binary
    -> inspect user-installed engine commands
    -> provision missing roles in a versioned user-local bundle
    -> validate every selected executable
    -> write absolute paths to runtime config
    -> write diagnostic installation state
    -> normal rendering performs no installation
```

The installer owns installation-time dependency resolution only. It does not alter the pre-display
safety gate, terminal input, child process, signals, resize forwarding, or display commit behavior.

## 2. User flow

Linux/macOS:

```bash
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
cd ptymark
bash scripts/install.sh
```

Windows PowerShell 7+:

```powershell
git clone --recurse-submodules https://github.com/iwashita-nozomu/ptymark.git
Set-Location ptymark
pwsh -File scripts/install.ps1
```

The top-level scripts perform these stages:

1. install the Rust binary with `cargo install --locked --force --path REPOSITORY`;
2. determine config, state, and managed-data locations;
3. inspect explicit selections and compatible commands already available on `PATH`;
4. install the managed bundle when one or more default roles remain unresolved;
5. call `ptymark install resolve` with concrete paths;
6. call `ptymark install status`.

The source-based alpha flow requires Rust/Cargo. A future release package may replace stage 1 without
changing stages 2 through 6.

## 3. Default renderer set

The managed fallback is a pinned, coherent set:

```text
bundle schema       1
Node.js             24.18.0
Mermaid CLI         11.16.0
MathJax             4.1.3
Puppeteer           25.2.1
terminal presenter  ptymark ANSI presenter v1
```

The roles are concrete:

```text
Mermaid slot
  mmdc protocol -> SVG

Math slot
  tex2svg FORMULA protocol -> SVG

Presenter slot
  fixed Chafa-compatible arguments + SVG file -> ANSI/Unicode bytes
```

The managed presenter deliberately implements only the fixed argument subset emitted by
`ConfiguredRenderer`. It is not a general Chafa replacement.

## 4. Resolution precedence

The top-level platform installer resolves each role in this order:

```text
explicit option
    > preserved valid selection when not re-probing
    > compatible command on PATH
    > managed bundle alias
    > built-in preview when managed mode is never
```

Explicit examples:

```bash
bash scripts/install.sh --mermaid /opt/homebrew/bin/mmdc
bash scripts/install.sh --math source
bash scripts/install.sh --presenter /usr/local/bin/chafa
```

```powershell
pwsh -File scripts/install.ps1 -Mermaid C:\Tools\mmdc.cmd
pwsh -File scripts/install.ps1 -Math preview
```

An explicitly requested external path must resolve successfully. An automatically unresolved role may
use the managed bundle or, when managed installation is disabled, the built-in preview.

A layout engine is never activated without a presenter. This prevents an SVG artifact from being
committed directly as terminal output.

## 5. Managed bundle isolation

Default roots:

```text
Linux
  ${XDG_DATA_HOME:-~/.local/share}/ptymark/renderer-bundles/<bundle-id>/

macOS
  ~/Library/Application Support/ptymark/renderer-bundles/<bundle-id>/

Windows
  %LOCALAPPDATA%\ptymark\renderer-bundles\<bundle-id>\
```

`<bundle-id>` contains the bundle schema and major pinned engine/runtime versions. Different sets can
coexist without replacing each other.

```text
<bundle-id>/
  bundle.toml
  bundle.stamp
  app/
    package.json
    package-lock.json
    node_modules/
    managed/mathjax-cli.mjs
    managed/ansi-presenter.mjs
  runtime/
    node-v24.18.0-<platform>-<arch>/
  cache/
    npm/
    puppeteer/
  bin/
    mmdc[.exe]
    tex2svg[.exe]
    chafa[.exe]
```

The bundle does not:

- modify the user's global npm prefix;
- add its `bin` directory to `PATH`;
- overwrite system Mermaid, MathJax, Node, Chafa, Chrome, or Edge;
- use a project-local `node_modules` directory outside its own root;
- run during a render request.

## 6. Native alias launcher

The three managed commands are copies or hard links of the installed native `ptymark` binary. Startup
examines the current executable name:

```text
mmdc[.exe]      -> managed Mermaid entrypoint
tex2svg[.exe]   -> managed MathJax entrypoint
chafa[.exe]     -> managed ANSI presenter entrypoint
ptymark[.exe]   -> normal CLI
```

The alias reads `<bundle-id>/bundle.toml`:

```toml
schema_version = 1
node_path = "/absolute/path/to/node"
app_root = "/absolute/path/to/bundle/app"
cache_root = "/absolute/path/to/bundle/cache/puppeteer"
browser_path = "/optional/absolute/path/to/browser"
browser_no_sandbox = false
```

The manifest is strict and versioned. Node, app, entrypoint, and optional browser paths are validated
before execution.

The alias invokes Node directly with a fixed role-specific JavaScript file and forwards the existing
fixed engine protocol arguments. It does not generate or execute a shell script, `.cmd` wrapper,
command string, pipe, redirect, or arbitrary argv template. This is important for TeX input on Windows,
where a batch wrapper could reinterpret metacharacters.

## 7. Runtime and browser provisioning

### 7.1 Node

The installer first accepts an exact system Node 24.18.0 plus npm. Otherwise it uses a private portable
Node runtime in the bundle.

When downloading Node:

1. choose the platform/architecture archive;
2. download it from the official versioned Node distribution path;
3. download the corresponding `SHASUMS256.txt`;
4. compare SHA-256 before extraction;
5. extract under the versioned bundle root.

Supported managed targets in the first implementation:

```text
Linux   x64, arm64
macOS   x64, arm64
Windows x64, arm64
```

### 7.2 JavaScript packages

The installer copies the repository's package manifest, lockfile, and fixed entrypoints into the bundle,
then runs:

```text
npm ci --omit=dev --no-audit --no-fund
```

npm's cache is redirected under the bundle root. The committed lockfile is the dependency source of
truth.

### 7.3 Browser

Selection options:

```text
explicit --browser / -Browser
existing browser with --skip-browser-download / -SkipBrowserDownload
Puppeteer-managed private browser under the bundle cache
```

An explicit browser remains owned and updated by the user or operating system. A Puppeteer-managed
browser remains private to the versioned bundle. Browser acquisition is permitted only during the
explicit install command.

## 8. Runtime configuration and state

Default configuration:

```text
Linux/macOS  ~/.config/ptymark/config.toml
Windows      %APPDATA%\ptymark\config.toml
```

Default state:

```text
Linux        ${XDG_STATE_HOME:-~/.local/state}/ptymark/install.toml
macOS        ~/.local/state/ptymark/install.toml
Windows      %LOCALAPPDATA%\ptymark\state\install.toml
```

The config contains the active runtime policy and absolute selected paths. The state file is diagnostic
evidence, not a second source of runtime policy.

```toml
schema_version = 1
ptymark_version = "0.1.0-alpha.1"
config_path = "C:\\Users\\user\\AppData\\Roaming\\ptymark\\config.toml"

[[components]]
role = "mermaid"
backend = "mermaid-cli"
active = true
origin = "explicit"
requested_path = "C:\\Users\\user\\AppData\\Local\\ptymark\\renderer-bundles\\...\\bin\\mmdc.exe"
resolved_path = "C:\\Users\\user\\AppData\\Local\\ptymark\\renderer-bundles\\...\\bin\\mmdc.exe"
```

Each component records:

- logical role;
- selected backend;
- active/inactive state;
- resolution origin;
- requested path;
- resolved absolute path;
- optional fallback note.

`ptymark install status` verifies the stored resolved executables. It does not silently replace them.
Replacement remains an explicit installer or `install resolve` operation.

## 9. Idempotence and replacement

An ordinary re-run preserves a valid existing config and its selected backends. It also preserves
unrelated detection, rendering, and cache settings.

Re-probe known system names, then managed fallbacks:

```bash
bash scripts/install.sh --reprobe
```

```powershell
pwsh -File scripts/install.ps1 -Reprobe
```

Force the coherent managed set:

```bash
bash scripts/install.sh --managed always
```

Disable managed provisioning:

```bash
bash scripts/install.sh --managed never
```

Offline reuse:

```bash
bash scripts/install.sh --offline
```

Reset project-owned configuration:

```bash
bash scripts/install.sh --reset
```

Print the final native resolver plan without writing config/state:

```bash
bash scripts/install.sh --dry-run
```

The managed app is reused when the bundle ID, package-lock digest, native launcher digest, Node path,
and browser identity match `bundle.stamp`. `--force-managed`/`-ForceManaged` rebuilds it.

## 10. Extension boundaries

### 10.1 Native executable resolution

```rust
pub trait ProgramResolver: Send + Sync {
    fn resolve(&self, configured: &Path) -> Result<PathBuf, InstallError>;
}
```

`PathProgramResolver` supports absolute paths and platform PATH conventions, including `PATHEXT` on
Windows. A future verified package location can implement this interface without changing config/state
serialization or render routing.

### 10.2 Managed source

Managed acquisition is deliberately outside normal `Installer<R>` resolution. A future source
implementation must produce the same outputs:

```text
absolute executable paths
strict bundle manifest
integrity evidence
versioned isolated root
idempotent installation result
```

Possible later sources include signed release archives or OS-specific package integrations. They must
not change the renderer protocol or terminal pipeline.

### 10.3 New engine role

Adding a role requires:

1. a user-visible syntax/use case;
2. a fixed executable or worker protocol;
3. a pinned default implementation where applicable;
4. platform installation and integrity rules;
5. a native launcher mapping or direct executable;
6. fallback behavior;
7. cache identity review;
8. Linux, macOS, and Windows contract tests;
9. a real integration smoke.

The installer is not a dynamic plugin registry.

## 11. Failure policy

| Situation | Automatic setup | Explicit/preserved external selection |
| --- | --- | --- |
| system engine missing | use managed role | installation error only when managed is disabled/unavailable |
| managed download unavailable | built-in preview when allowed | error for an explicitly required managed/external role |
| checksum mismatch | fail before extraction | fail |
| lockfile install failure | fail without activating partial bundle | fail |
| presenter unavailable | do not activate SVG engine | fail when explicitly required |
| existing config invalid | fail without replacement | fail |
| state missing | `install status` reports error | rerun installer |
| selected executable removed later | status reports `missing` | explicit re-resolution required |
| config/state write failure | no successful install result | correct path/permissions and rerun |

Config and state are written via same-directory temporary files and renamed after flush. Bundle stamps
are written only after the app install succeeds.

## 12. Security boundary

- downloads occur only during an explicit install command;
- normal render requests never run npm, Cargo, curl, Invoke-WebRequest, or browser installation;
- official Node archives are SHA-256 verified;
- npm resolution is lockfile-based;
- managed files remain under a versioned user data root;
- no global package or PATH mutation occurs;
- managed aliases are native binaries, not shell wrappers;
- renderer configuration stores no shell command strings or arbitrary argument templates;
- manifest and executable paths are validated before invocation;
- project-local configuration is not auto-loaded;
- render failures restore exact source unless strict mode is enabled.

## 13. Verification

Rust contracts cover:

- first-install resolution;
- missing-presenter fallback;
- explicit missing-engine failure;
- one-slot replacement with unrelated settings preserved;
- reset and dry-run behavior;
- installation-state round trip;
- re-probe replacing stale absolute paths;
- Windows absolute path and `PATHEXT` behavior;
- managed alias role dispatch.

GitHub Actions additionally covers:

```text
Ubuntu/macOS/Windows
  format + Clippy + all Rust tests

Windows managed setup
  PowerShell parse
  pinned Node
  installed Edge selection
  isolated npm bundle
  native .exe aliases
  config/state verification
  Mermaid render
  MathJax render

Canonical Docker
  shell syntax + ShellCheck
  Node entrypoint syntax
  installer contract smoke
  isolated managed engine smoke
  real renderer checks
```
