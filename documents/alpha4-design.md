# v0.1.0-alpha.4 design: typed configuration and bounded standard infrastructure

<!--
@dependency-start
contract design
responsibility Defines alpha.4 configuration ownership, resolution, migration, and infrastructure simplification.
upstream design ../README.md product contract
upstream design ../documents/ptymark-design.md pre-display architecture
upstream issue https://github.com/iwashita-nozomu/ptymark/issues/141 alpha.4 tracker
downstream implementation ../src/config.rs user and resolved configuration
downstream implementation ../src/limits.rs internal safety floor
downstream implementation ../src/install.rs installation state and commit protocol
downstream implementation ../src/cli.rs typed command model
downstream implementation ../src/runtime.rs resolved runtime composition
@dependency-end
-->

## Purpose

`v0.1.0-alpha.4` turns the provisional alpha configuration into a typed,
extensible ownership model and removes generic infrastructure that Ptymark should
not maintain itself.

The central rule is:

```text
user TOML describes stable intent
installation state records machine-local resolution
session overrides describe one invocation
internal constants enforce the non-negotiable safety floor
```

A path, timeout, byte limit, or runtime fact is not automatically a user setting
merely because the program uses it.

## Four ownership layers

### 1. User configuration

The user-authored TOML owns choices that remain meaningful across machines:

- named profiles and the default profile;
- render versus exact-source session mode;
- strict versus source-restoring renderer failure behavior;
- Mermaid and math detector enablement;
- presentation preference: `auto`, `symbols`, `plain`, or `source`;
- color preference: `auto`, `always`, or `never`;
- fallback column width when no terminal size is available;
- memory-cache enablement and bounded local budgets;
- renderer/provider intent: `auto`, `preview`, `source`, `managed`, or `external`;
- an executable program only when the user explicitly selects `external`.

It does **not** contain discovered executable paths, managed-bundle locations,
installation ownership, temporary directories, process IDs, browser cache paths,
or hard safety limits.

### 2. Installation state

The installation snapshot is machine-local internal state. It records:

- the Ptymark version that produced it;
- the associated user-config path;
- a SHA-256 digest of normalized user intent;
- resolved executable paths and resolution origin;
- active/inactive component ownership;
- bounded, non-source-bearing notes needed for diagnosis.

The runtime uses installation state only when both the config path and normalized
config digest match. A partially committed or stale state file is therefore not
mistaken for the current user configuration.

### 3. Session overrides

The typed CLI produces one per-invocation override object for:

- source or safe mode;
- private mode;
- strictness;
- cache bypass;
- color request;
- width override;
- selected profile;
- intentional nested-session permission.

Clap rejects conflicting source/safe requests before configuration loading,
terminal mutation, or child launch. Child arguments after `--` remain
`OsString` values and are never interpreted as a shell command string.

### 4. Internal policy

`src/limits.rs` owns versioned hard bounds such as:

- the shared external render/presentation deadline;
- renderer stdout/stderr, artifact, and presentation byte caps;
- pending terminal-output hard cap;
- maximum semantic block size;
- OpenMath input/depth/node caps;
- SVG parser node cap;
- terminal-control sequence cap;
- process polling, resize polling, and platform drain timing.

These limits are intentionally absent from user TOML in alpha.4. A future stable
schema may expose softer preferences, but it may not weaken this internal floor.

## Schema v2

Canonical shape:

```toml
schema_version = 2
default_profile = "default"

[profiles.default.session]
mode = "render"
strict = false

[profiles.default.detection]
mermaid = true
math = true

[profiles.default.presentation]
mode = "auto"
color = "auto"
fallback_columns = 80

[profiles.default.cache]
backend = "memory"
max_entries = 128
max_bytes = 33554432

[profiles.default.engines.mermaid]
provider = "auto"

[profiles.default.engines.math]
provider = "auto"

[profiles.default.engines.presenter]
provider = "auto"
```

### Provider meanings

`auto`
: Use matching current installation state when available; otherwise use a
  dependency-free built-in path. Installer PATH discovery remains internal.

`preview`
: Use the built-in selectable terminal preview.

`source`
: Preserve exact fenced source for the semantic role.

`managed`
: Require a compatible managed alias recorded in matching installation state.
  No managed absolute path is serialized to user TOML.

`external`
: Use the explicitly user-selected `program`. This is the only provider for
  which `program` is valid.

## Resolution pipeline

```text
TOML source
  -> schema discriminator
  -> UserConfig v2 (or deterministic v1 migration)
  -> profile selection
  + matching InstallState v2
  + typed SessionOverrides
  -> immutable resolved Config
  -> PipelineFactory
```

The stream loop and renderer never read TOML directly. Doctor, preview, filtered
run, and native PTY/ConPTY use the same resolver.

## Schema-v1 compatibility

Alpha.4 reads schema v1 and maps it into one `default` profile. It does not
reinterpret unknown fields or silently write the file.

```bash
ptymark --config old.toml config migrate
ptymark --config old.toml config migrate --write
```

The first command prints normalized schema-v2 TOML. `--write` performs an
atomic replacement. `config show` always prints normalized user intent and does
not expose machine-local resolved paths or internal constants.

## Canonical command model

The canonical interactive form is:

```bash
ptymark shell [OPTIONS] -- COMMAND [ARG...]
```

The alpha.3 form remains a compatibility alias for alpha.4:

```bash
ptymark [OPTIONS] -- COMMAND [ARG...]
```

The alias is normalized before Clap parsing and may be removed only through the
compatibility-entrypoint policy in #137.

## Standard infrastructure choices

Alpha.4 delegates generic mechanics to maintained crates while keeping
Ptymark-specific policy local:

| Concern | Standard component | Ptymark-owned policy |
| --- | --- | --- |
| CLI grammar/help | Clap derive | compatibility alias, session semantics, child argv boundary |
| TOML | Serde + `toml` | schema, validation, resolution, migration |
| Doctor JSON | Serde + `serde_json` | public-safe typed report and redaction |
| XML tokenization | `roxmltree` | bounded OpenMath model and SVG compatibility checks |
| LRU mechanics | `lru` | admission, logical byte budget, privacy modes, cache identity |
| OS directories | `directories` | `PTYMARK_*` precedence and source-only lifecycle |
| executable lookup | `which` | absolute/bare restriction and Windows wrapper rejection |
| temporary files | `tempfile` | ownership, no-clobber, digest/commit protocol |
| config identity | SHA-256 | normalized-user-intent scope and stale-state rejection |

The generic process supervisor remains local in alpha.4 because Unix process
groups, Windows descendant cleanup, shared attempt deadlines, and pending-output
cancellation must not be weakened. It remains the focused follow-up in #132.

## Terminal and renderer safety

- control-sequence parsing state is explicitly bounded;
- oversized or ambiguous control traffic switches to byte-exact raw handling;
- semantic detection never receives terminal-control or alternate-screen bytes;
- OpenMath parsing disables DTD/entity resolution and remains bounded;
- SVG output must be well-formed, have an `svg` root, and use the SVG namespace;
- invalid/partial artifacts are not presented or cached;
- source/safe/private behavior remains unchanged;
- non-strict failures restore exact source in order;
- successful renderer output alone receives interactive CRLF normalization.

## Installation commit protocol

Config and state cannot be replaced atomically by one portable filesystem
operation. Alpha.4 therefore uses a recoverable two-file protocol:

1. serialize and validate normalized user config and state;
2. put the config SHA-256 digest into state;
3. stage and sync both files in their destination directories;
4. commit state first;
5. commit config;
6. roll state back if config replacement fails.

A process interruption after step 4 leaves a digest mismatch. Runtime resolution
ignores that state, so the incomplete installation cannot activate newly
resolved engines.

## Acceptance

- schema v1 and v2 produce deterministic resolved behavior;
- only user intent appears in `config show` and canonical examples;
- installer-discovered paths appear only in install state/inspection output;
- internal hard limits are not user-tunable;
- typed CLI help and validation replace hand-written option loops;
- Linux, macOS, Windows, Docker, real PTY/ConPTY, managed renderer, source/safe/private,
  doctor/redaction, installer, and shell-coexistence tests remain green;
- the source-only release workflow publishes an immutable tag and notes with
  zero project-uploaded executable assets.

## Deferred

- persistent renderer workers and a shared process-supervisor library decision;
- complete stable config provenance/project trust/inheritance;
- copyable multi-variant structured artifacts from #124;
- persistent cache;
- terminal image protocols;
- signed native/package-manager distribution.
