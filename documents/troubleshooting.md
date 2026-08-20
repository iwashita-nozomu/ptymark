<!--
@dependency-start
contract reference
responsibility Documents public-safe diagnosis, recovery, and support-report handling for doctor v1 and bounded renderer failures.
upstream implementation ../src/doctor/mod.rs implements doctor and support-report behavior
upstream implementation ../src/managed_launcher.rs implements bounded managed runtime probes
upstream implementation ../src/render.rs enforces bounded rendering and exact-source recovery
downstream environment ../.github/ISSUE_TEMPLATE/bug-report.yml routes redacted public support intake
@dependency-end
-->

# Troubleshooting and public-safe support reports

`ptymark 0.1.0-alpha.2` adds one public-safe diagnosis path:

```text
ptymark doctor
ptymark doctor --json
ptymark doctor --support-report PATH
ptymark doctor --config PATH
```

Doctor does not install or download dependencies, use the network, start a PTY/ConPTY
child, or mutate configuration, installation state, cache, shell profiles, terminal
configuration, or other user files. Arbitrary external renderer and presenter commands
are still inspected by file resolution only. When an executable belongs to a managed
bundle, doctor additionally runs one fixed Mermaid, MathJax, or presenter sample under
an eight-second monotonic deadline so that file presence cannot be mistaken for runtime
readiness.

## Status and exit codes

| Status | Exit code | Meaning |
| --- | ---: | --- |
| `ready` | `0` | The selected configuration is usable; every active managed component also passed its bounded sample. |
| `degraded` | `10` | Ptymark remains usable through an explicit fallback or without an optional capability. |
| `unusable` | `20` | The selected configuration, required host, or strict path cannot operate. |

Syntax and CLI usage errors retain exit code `2`.

## Diagnosis flow

```mermaid
flowchart TD
    S[Rendering or terminal problem] --> D[Run ptymark doctor]
    D --> R{Status}
    R -->|ready| P[Inspect the affected block or presenter]
    R -->|degraded| F[Use the safe fallback and follow the remedy]
    R -->|unusable| C[Correct config, install state, or host]
    P --> J[Attach the redacted doctor v1 report]
    F --> J
    C --> J
```

## Managed Chromium cannot start on Ubuntu or WSL

The managed bundle report deliberately has two independent states:

- `state` covers executable, manifest, and configured browser-file presence;
- `runtime_state` covers the bounded Mermaid, MathJax, or presenter launch.

A bundle may therefore show `state: ready` and `runtime_state: missing-libraries`.
The stable finding is `browser.runtime_libraries_missing`; its `libraries` evidence
contains validated `.so` basenames only. For the common fresh Ubuntu 22.04/24.04 and
WSL NSS/NSPR failure, run:

```bash
sudo apt-get update
sudo apt-get install --yes libnspr4 libnss3
ptymark doctor
```

This resolves `libnspr4.so`, `libnss3.so`, `libnssutil3.so`, and `libsmime3.so`.
Install the distribution package providing any additional listed library, then rerun
doctor. `ptymark install status` uses the same bounded readback and reports an unusable
managed component as `missing` rather than `ready`. Selecting an explicit compatible
system Chromium remains supported; the probe follows the browser path recorded in the
managed manifest and does not replace it.

The probe fixes `TERM` and removes `TMUX` from its child environment, captures bounded
output in private temporary files, and never serializes raw Chromium/Puppeteer stderr.
Its result is therefore independent of whether doctor was invoked in tmux, a normal PTY,
or redirected output.

## Redaction contract

The JSON root schema is `ptymark.doctor.v1`. Public-by-default human, JSON, and support-report output excludes or redacts:

- semantic source and excerpts;
- child environment and command history;
- credentials, tokens, cookies, and configured secret values;
- raw renderer or browser stderr that may echo source or paths;
- home, XDG, and platform application-data prefixes where practical;
- terminal control bytes and invalid byte sequences.

Do not paste an unrestricted environment dump or raw renderer stderr to compensate for omitted data. Security vulnerabilities or accidental credential exposure belong in the private GitHub Security Advisory flow.

## Immediate recovery modes

These modes retain the real PTY/ConPTY host while changing only rendering policy for the invocation:

```text
ptymark --source -- COMMAND   # detect blocks, display exact source
ptymark --safe -- COMMAND     # bypass semantic detection and external rendering
ptymark --private -- COMMAND  # keep rendering, disable cache/persistent diagnostics
```

The same options work with `preview` and the pipe-oriented `run -- COMMAND` path where applicable. `--source` and `--safe` are mutually exclusive; `--private` may be combined with either.

## Bounded renderer recovery

Each normal external render/presentation attempt has a ten-second monotonic hard deadline and bounded output. Later terminal output held behind one unresolved semantic block is limited to one MiB. In non-strict mode, timeout, output-limit, process, invalid-artifact, or presentation failure restores exact source and then releases later output in original order. Failed or cancelled results are not cached.

Given:

```text
ordinary A
semantic block B
ordinary C
```

visible order remains:

```text
A
rendered result or exact source for B
C
```

A renderer timeout never terminates the user's PTY/ConPTY child process.

## Attaching a report

Generate a new report path; existing files are not overwritten:

```bash
ptymark doctor --support-report ./ptymark-support.json
```

```powershell
ptymark doctor --support-report .\ptymark-support.json
```

Review the report before attaching it. When doctor cannot start, report the exact `ptymark --version`, operating system/architecture, invocation path, and a safe minimal reproduction instead.
