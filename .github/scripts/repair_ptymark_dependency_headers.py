# @dependency-start
# contract tool
# responsibility Repairs ptymark dependency manifests to the pinned AgentCanon grammar.
# upstream design ../../vendor/agent-canon/documents/dependency-manifest-design.md manifest DSL
# downstream implementation ../workflows/ptymark-release-metadata.yml bounded release repair
# @dependency-end

from pathlib import Path

S = {
'.github/workflows/ci.yml': ('workflow','Runs repository CI and routes inherited gates.',[
'upstream implementation ../../tools/ci/run_all_checks.sh repository CI entrypoint','upstream implementation ../../Makefile CI target contract','upstream environment ../../docker/install_python_dependencies.sh Python dependency installer','upstream implementation ../scripts/checkout_agent_canon_submodule.sh AgentCanon checkout gate','downstream implementation ./ptymark-ci.yml product validation workflow']),
'.github/workflows/ptymark-ci.yml': ('workflow','Runs the cross-platform ptymark acceptance matrix.',[
'upstream design ../../documents/ptymark-design.md terminal-safety architecture','upstream environment ../../rust-toolchain.toml supported Rust toolchain','downstream design ../../verification/README.md merge evidence policy','downstream implementation ../../tests/verification_manifest_contract.rs catalog validation']),
'Cargo.toml': ('configuration','Defines package metadata, dependencies, compatibility, and release profile.',[
'upstream environment rust-toolchain.toml compiler pin','downstream implementation Cargo.lock resolved dependency graph','downstream implementation src/lib.rs library surface','downstream implementation tests/cli_contract.rs CLI validation']),
'README.md': ('design','Provides the user-facing installation, configuration, safety, and usage entrypoint.',[
'upstream design documents/ptymark-design.md architecture contract','upstream design documents/ptymark-installer.md installation contract','upstream design documents/shell-plugin-compatibility.md coexistence evidence','downstream implementation src/cli.rs command surface','downstream implementation src/install.rs installation state','downstream implementation scripts/installer.sh setup frontend','downstream implementation tests/cli_contract.rs user-facing validation','downstream environment .github/workflows/ptymark-ci.yml acceptance matrix']),
'distribution/install.sh': ('implementation','Installs one extracted Unix package without a source checkout.',[
'upstream design ../documents/ptymark-installer.md package installation contract','upstream implementation ../scripts/package-release.sh package assembly','downstream implementation ../tests/install_smoke.sh installation validation']),
'docker/ptymark-compose.yaml': ('environment','Runs the canonical ptymark development image.',[
'upstream environment ./ptymark.Dockerfile container toolchain','downstream environment ../ptymark.mk development commands']),
'docker/ptymark.Dockerfile': ('environment','Builds the canonical ptymark validation environment.',[
'upstream environment ./ptymark-versions.env runtime versions','upstream environment ../renderers/package-lock.json renderer dependency lock','downstream environment ../.github/workflows/ptymark-ci.yml canonical checks']),
'documents/README.md': ('design','Indexes repository-local design, verification, compatibility, and shared policy.',[
'upstream design ../vendor/agent-canon/documents/SHARED_RUNTIME_SURFACES.md shared ownership policy','upstream design ../vendor/agent-canon/documents/agent-canon-parent-repo-latest-checklist.md parent readiness policy','downstream design ./ptymark-design.md architecture contract','downstream design ./ptymark-installer.md installation contract','downstream design ./shell-plugin-compatibility.md coexistence evidence','downstream design ../verification/README.md verification policy']),
'documents/ptymark-design.md': ('design','Defines rendering architecture, terminal safety, and extension ownership.',[
'upstream design ../README.md user-facing behavior','downstream implementation ../src/pipeline.rs display pipeline','downstream implementation ../src/terminal.rs terminal safety gate']),
'documents/ptymark-installer.md': ('design','Defines installer, managed renderer, path, and state behavior.',[
'upstream design ../README.md installation journeys','downstream implementation ../src/install.rs state resolution','downstream implementation ../scripts/installer.sh platform setup','downstream implementation ../tests/install_smoke.sh installation validation']),
'documents/shell-plugin-compatibility.md': ('reference','Maps shell behavior to byte-preservation and coexistence evidence.',[
'upstream design ./ptymark-design.md terminal-safety boundary','downstream implementation ../src/terminal.rs protected output handling','downstream implementation ../tests/shell_plugin_compatibility.rs behavior validation','downstream implementation ../tests/shell_profile_coexistence.sh profile validation']),
'documents/filtered-command.md': ('design','Defines pipe-oriented stdout filtering.',[
'upstream design ./ptymark-design.md shared pipeline','downstream implementation ../src/filtered_run.rs command execution','downstream implementation ../tests/filtered_run_contract.rs behavior validation']),
'documents/interactive-session.md': ('design','Defines native PTY and ConPTY session behavior.',[
'upstream design ./ptymark-design.md terminal-safety boundary','downstream implementation ../src/interactive.rs session composition','downstream implementation ../tests/interactive_pty_contract.rs native runtime validation']),
'examples/README.md': ('reference','Explains supported configuration and WezTerm examples.',[
'upstream design ../README.md user-facing surface','upstream design ../documents/ptymark-design.md architecture contract','downstream implementation ../tests/plugin_smoke.lua executable examples']),
'examples/external-engines.toml': ('configuration','Demonstrates explicit external renderer selection.',[
'upstream design ../documents/ptymark-installer.md executable discovery contract','downstream implementation ../src/config.rs configuration parser','downstream implementation ../src/engine.rs selected engine roles']),
'examples/ptymark.toml': ('configuration','Provides a minimal portable configuration fixture.',[
'upstream design ../README.md documented configuration','downstream implementation ../src/config.rs configuration parser','downstream implementation ../tests/cli_contract.rs fixture validation']),
'rust-toolchain.toml': ('environment','Pins the Rust compiler and required components.',[
'upstream environment Cargo.toml minimum Rust version','downstream environment .github/workflows/ptymark-ci.yml product checks','downstream environment docker/ptymark.Dockerfile canonical image']),
'scripts/check-ptymark-renderers.sh': ('tool','Checks renderer executables and bounded direct rendering.',[
'upstream implementation ../src/engine.rs executable checks','upstream implementation ../src/managed_launcher.rs managed role launch','downstream implementation ../tests/managed_renderer_smoke.sh renderer validation']),
'scripts/install-managed-bundle.sh': ('implementation','Installs the pinned private renderer bundle on Unix.',[
'upstream design ../documents/ptymark-installer.md bundle isolation','upstream environment ../renderers/managed-bundle.env runtime pins','upstream environment ../renderers/package-lock.json package lock','downstream implementation ./installer.sh role selection','downstream implementation ../tests/managed_renderer_smoke.sh bundle validation']),
'scripts/install.sh': ('implementation','Preserves the former installer command as a compatibility wrapper.',[
'upstream design ../README.md documented compatibility command','upstream implementation ./installer.sh canonical installer','downstream implementation ../tests/install_smoke.sh wrapper validation']),
'scripts/installer.sh': ('implementation','Coordinates Unix and Windows-Bash installation.',[
'upstream design ../documents/ptymark-installer.md installation semantics','upstream implementation ../src/install.rs state resolution','downstream implementation ../distribution/install.sh package entrypoint','downstream implementation ../tests/install_smoke.sh installer validation']),
'scripts/package-release.sh': ('implementation','Assembles and smoke-tests one Unix release archive.',[
'upstream environment ../Cargo.toml package version','upstream design ../README.md documented package contents','downstream environment ../.github/workflows/ptymark-ci.yml package jobs','downstream implementation ../tests/install_smoke.sh installation expectations']),
'tests/install_smoke.sh': ('test','Exercises source and package-local installation.',[
'upstream implementation ../scripts/installer.sh source installation','upstream implementation ../distribution/install.sh package installation','downstream environment ../.github/workflows/ptymark-ci.yml test execution']),
'tests/managed_renderer_smoke.sh': ('test','Proves isolated Mermaid, math, and presenter execution.',[
'upstream implementation ../scripts/install-managed-bundle.sh bundle installation','upstream implementation ../src/managed_launcher.rs role execution','downstream environment ../.github/workflows/ptymark-ci.yml evidence recording']),
'tests/shell_profile_coexistence.sh': ('test','Proves installers do not mutate shell profiles.',[
'upstream implementation ../scripts/installer.sh shell-facing setup','upstream implementation ../src/terminal.rs terminal ownership','downstream environment ../.github/workflows/ptymark-ci.yml coexistence checks']),
'tests/verification_manifest_contract.rs': ('test','Validates the verification catalog and referenced paths.',[
'upstream design ../verification/manifest.toml required checks','upstream design ../verification/README.md evidence policy','downstream environment ../.github/workflows/ptymark-ci.yml supported-platform execution']),
'verification/README.md': ('reference','Explains verification evidence, reproduction, and merge policy.',[
'upstream design ./manifest.toml required checks','upstream implementation ../.github/workflows/ptymark-ci.yml product evidence','upstream implementation ../.github/workflows/ci.yml repository evidence','downstream implementation ../tests/verification_manifest_contract.rs drift prevention']),
'verification/manifest.toml': ('registry','Defines every merge-relevant verification item.',[
'upstream implementation ../.github/workflows/ptymark-ci.yml product checks','upstream implementation ../.github/workflows/ci.yml repository checks','downstream implementation ../tests/verification_manifest_contract.rs catalog validation','downstream design ./README.md evidence documentation']),
}

def header(path, contract, responsibility, edges):
    body=['@dependency-start',f'contract {contract}',f'responsibility {responsibility}',*edges,'@dependency-end']
    if path.suffix=='.md': return ['<!--',*body,'-->']
    prefix='// ' if path.suffix=='.rs' else '# '
    return [prefix+x for x in body]

def rewrite(path, spec):
    lines=path.read_text(encoding='utf-8').splitlines()
    start=next((i for i,x in enumerate(lines) if '@dependency-start' in x),None)
    if start is not None:
        end=next(i for i in range(start,len(lines)) if '@dependency-end' in lines[i])
        if start and lines[start-1].strip()=='<!--': start-=1
        if end+1<len(lines) and lines[end+1].strip()=='-->': end+=1
        del lines[start:end+1]
    insert=1 if lines and (lines[0].startswith('#!') or lines[0].startswith('# syntax=docker/dockerfile') or (path.suffix=='.md' and lines[0].startswith('# '))) else 0
    rest=lines[insert:]
    while rest and not rest[0].strip(): rest.pop(0)
    out=lines[:insert]+([''] if insert else [])+header(path,*spec)+['']+rest
    path.write_text('\n'.join(out).rstrip()+'\n',encoding='utf-8')

for relative,spec in S.items():
    path=Path(relative)
    if path.exists(): rewrite(path,spec)
