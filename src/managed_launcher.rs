use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const MANAGED_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const MANAGED_RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_secs(8);
pub const BROWSER_RUNTIME_LIBRARIES_MISSING: &str =
    "browser.runtime_libraries_missing";
pub const BROWSER_RUNTIME_LAUNCH_FAILED: &str = "browser.runtime_launch_failed";
pub const BROWSER_RUNTIME_TIMEOUT: &str = "browser.runtime_timeout";
const MANAGED_RUNTIME_PROBE_CAPTURE_BYTES: u64 = 4096;
#[cfg(target_os = "linux")]
const LINUX_LIBRARY_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManagedRole {
    Mermaid,
    Math,
    Presenter,
}

impl ManagedRole {
    fn from_executable(path: &Path) -> Option<Self> {
        let name = path.file_stem()?.to_string_lossy().to_ascii_lowercase();
        match name.as_str() {
            "mmdc" | "ptymark-mmdc" => Some(Self::Mermaid),
            "tex2svg" | "ptymark-tex2svg" => Some(Self::Math),
            "chafa" | "ptymark-presenter" => Some(Self::Presenter),
            _ => None,
        }
    }

    const fn script_suffix(self) -> &'static str {
        match self {
            Self::Mermaid => "node_modules/@mermaid-js/mermaid-cli/src/cli.js",
            Self::Math => "managed/mathjax-cli.mjs",
            Self::Presenter => "managed/ansi-presenter.mjs",
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManagedBundleManifest {
    schema_version: u32,
    node_path: PathBuf,
    app_root: PathBuf,
    cache_root: PathBuf,
    #[serde(default)]
    browser_path: Option<PathBuf>,
    #[serde(default)]
    puppeteer_config_path: Option<PathBuf>,
    #[serde(default)]
    browser_no_sandbox: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedBundleStatus {
    Compatible,
    IncompatibleSchema { found: u32, expected: u32 },
    InvalidPath { field: &'static str, reason: String },
}

impl ManagedBundleStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::IncompatibleSchema { .. } => "incompatible-schema",
            Self::InvalidPath { .. } => "invalid-path",
        }
    }

    pub const fn is_compatible(&self) -> bool {
        matches!(self, Self::Compatible)
    }

    fn execution_error(&self) -> Option<String> {
        match self {
            Self::Compatible => None,
            Self::IncompatibleSchema { found, expected } => Some(format!(
                "unsupported managed bundle schema {found}; expected {expected}"
            )),
            Self::InvalidPath { field, reason } => {
                Some(format!("managed bundle {field} is invalid: {reason}"))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManagedRuntimeStatus {
    Ready,
    MissingLibraries { libraries: Vec<String> },
    LaunchFailed,
    TimedOut,
    InvalidArtifact,
}

impl ManagedRuntimeStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::MissingLibraries { .. } => "missing-libraries",
            Self::LaunchFailed => "launch-failed",
            Self::TimedOut => "timeout",
            Self::InvalidArtifact => "invalid-artifact",
        }
    }

    pub const fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    pub fn missing_libraries(&self) -> &[String] {
        match self {
            Self::MissingLibraries { libraries } => libraries,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug)]
struct LoadedManifest {
    manifest: ManagedBundleManifest,
    status: ManagedBundleStatus,
}

impl LoadedManifest {
    fn read(path: &Path) -> Result<Self, String> {
        let source = fs::read_to_string(path).map_err(|error| {
            format!(
                "cannot read managed bundle manifest `{}`: {error}",
                path.display()
            )
        })?;
        let manifest: ManagedBundleManifest = toml::from_str(&source).map_err(|error| {
            format!(
                "cannot parse managed bundle manifest `{}`: {error}",
                path.display()
            )
        })?;
        let status = validate_manifest(&manifest);
        Ok(Self { manifest, status })
    }

    fn into_execution_manifest(self) -> Result<ManagedBundleManifest, String> {
        if let Some(error) = self.status.execution_error() {
            return Err(error);
        }
        Ok(self.manifest)
    }
}

fn validate_manifest(manifest: &ManagedBundleManifest) -> ManagedBundleStatus {
    if manifest.schema_version != MANAGED_BUNDLE_SCHEMA_VERSION {
        return ManagedBundleStatus::IncompatibleSchema {
            found: manifest.schema_version,
            expected: MANAGED_BUNDLE_SCHEMA_VERSION,
        };
    }
    for (field, path, kind) in [
        ("node_path", manifest.node_path.as_path(), PathKind::File),
        ("app_root", manifest.app_root.as_path(), PathKind::Directory),
        (
            "cache_root",
            manifest.cache_root.as_path(),
            PathKind::Absolute,
        ),
    ] {
        if let Some(reason) = invalid_path_reason(path, kind) {
            return ManagedBundleStatus::InvalidPath { field, reason };
        }
    }
    if let Some(path) = manifest.browser_path.as_deref()
        && let Some(reason) = invalid_path_reason(path, PathKind::File)
    {
        return ManagedBundleStatus::InvalidPath {
            field: "browser_path",
            reason,
        };
    }
    if let Some(path) = manifest.puppeteer_config_path.as_deref()
        && let Some(reason) = invalid_path_reason(path, PathKind::File)
    {
        return ManagedBundleStatus::InvalidPath {
            field: "puppeteer_config_path",
            reason,
        };
    }
    ManagedBundleStatus::Compatible
}

#[derive(Clone, Copy)]
enum PathKind {
    Absolute,
    File,
    Directory,
}

fn invalid_path_reason(path: &Path, kind: PathKind) -> Option<String> {
    if !path.is_absolute() {
        return Some("path must be absolute".to_owned());
    }
    match kind {
        PathKind::Absolute => None,
        PathKind::File if !path.is_file() => {
            Some(format!("path does not name a file: `{}`", path.display()))
        }
        PathKind::Directory if !path.is_dir() => Some(format!(
            "path does not name a directory: `{}`",
            path.display()
        )),
        PathKind::File | PathKind::Directory => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedBundleInspection {
    pub manifest_path: PathBuf,
    pub schema_version: u32,
    pub node_path: PathBuf,
    pub browser_path: Option<PathBuf>,
    pub browser_available: Option<bool>,
    pub status: ManagedBundleStatus,
    /// Alpha.3 compatibility field. New consumers should use `status`.
    pub complete: bool,
}

/// Inspect the manifest next to a managed renderer alias without starting
/// Node, Chromium, an engine, or a presenter. Non-managed executables return
/// `None`.
pub fn inspect_managed_alias(executable: &Path) -> Option<Result<ManagedBundleInspection, String>> {
    ManagedRole::from_executable(executable)?;
    let bin_root = executable.parent()?;
    let bundle_root = bin_root.parent()?;
    let manifest_path = bundle_root.join("bundle.toml");
    if !manifest_path.is_file() {
        return None;
    }
    Some(LoadedManifest::read(&manifest_path).map(|loaded| {
        let complete = loaded.status.is_compatible();
        ManagedBundleInspection {
            manifest_path,
            schema_version: loaded.manifest.schema_version,
            node_path: loaded.manifest.node_path,
            browser_path: loaded.manifest.browser_path.clone(),
            browser_available: loaded.manifest.browser_path.as_deref().map(Path::is_file),
            status: loaded.status,
            complete,
        }
    }))
}

/// Execute one fixed, minimal managed-renderer sample under a monotonic timeout.
///
/// The result contains only a stable status and validated shared-library
/// basenames. Raw stdout/stderr, semantic input, environment values, and home
/// paths never leave this module.
pub fn probe_managed_alias(executable: &Path) -> Option<Result<ManagedRuntimeStatus, String>> {
    probe_managed_alias_with_timeout(executable, MANAGED_RUNTIME_PROBE_TIMEOUT)
}

fn probe_managed_alias_with_timeout(
    executable: &Path,
    timeout: Duration,
) -> Option<Result<ManagedRuntimeStatus, String>> {
    let role = ManagedRole::from_executable(executable)?;
    let inspection = match inspect_managed_alias(executable)? {
        Ok(inspection) => inspection,
        Err(error) => return Some(Err(error)),
    };
    if !inspection.complete {
        return Some(Err(format!(
            "managed bundle is {}",
            inspection.status.as_str()
        )));
    }

    Some((|| {
        let root = tempfile::Builder::new()
            .prefix("ptymark-runtime-probe-")
            .tempdir()
            .map_err(|error| format!("cannot create managed runtime probe directory: {error}"))?;
        let mut command = Command::new(executable);
        let artifact = match role {
            ManagedRole::Mermaid => {
                let input = root.path().join("probe.mmd");
                let output = root.path().join("probe.svg");
                fs::write(&input, b"flowchart LR\n  A --> B\n").map_err(|error| {
                    format!("cannot stage managed Mermaid probe input: {error}")
                })?;
                command
                    .arg("--input")
                    .arg(&input)
                    .arg("--output")
                    .arg(&output);
                ProbeArtifact::SvgFile(output)
            }
            ManagedRole::Math => {
                command.arg("E = mc^2");
                ProbeArtifact::StdoutSvg
            }
            ManagedRole::Presenter => {
                let input = root.path().join("probe.svg");
                fs::write(
                    &input,
                    br#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><rect width="2" height="2" fill="white"/></svg>"#,
                )
                .map_err(|error| format!("cannot stage managed presenter probe input: {error}"))?;
                command
                    .args([
                        "--format",
                        "symbols",
                        "--probe",
                        "off",
                        "--polite",
                        "on",
                        "--relative",
                        "off",
                        "--animate",
                        "off",
                        "--colors",
                        "none",
                        "--size",
                        "8x",
                    ])
                    .arg(&input);
                ProbeArtifact::StdoutNonempty
            }
        };
        command.env("TERM", "dumb").env_remove("TMUX");

        let output = run_bounded(command, timeout)?;
        if output.timed_out {
            return Ok(ManagedRuntimeStatus::TimedOut);
        }
        if !output.success {
            let mut libraries = extract_missing_libraries(&output.stderr);
            if let Some(browser) = inspection.browser_path.as_deref() {
                libraries.extend(missing_linux_libraries(browser));
            }
            libraries.sort();
            libraries.dedup();
            return Ok(if libraries.is_empty() {
                ManagedRuntimeStatus::LaunchFailed
            } else {
                ManagedRuntimeStatus::MissingLibraries { libraries }
            });
        }

        if artifact.is_valid(&output.stdout)? {
            Ok(ManagedRuntimeStatus::Ready)
        } else {
            Ok(ManagedRuntimeStatus::InvalidArtifact)
        }
    })())
}

#[derive(Debug)]
enum ProbeArtifact {
    SvgFile(PathBuf),
    StdoutSvg,
    StdoutNonempty,
}

impl ProbeArtifact {
    fn is_valid(&self, stdout: &[u8]) -> Result<bool, String> {
        match self {
            Self::SvgFile(path) => Ok(read_prefix(path).is_ok_and(|bytes| contains_svg(&bytes))),
            Self::StdoutSvg => Ok(contains_svg(stdout)),
            Self::StdoutNonempty => Ok(stdout.iter().any(|byte| !byte.is_ascii_whitespace())),
        }
    }
}

#[derive(Debug)]
struct BoundedOutput {
    success: bool,
    timed_out: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_bounded(mut command: Command, timeout: Duration) -> Result<BoundedOutput, String> {
    let mut stdout = tempfile::tempfile()
        .map_err(|error| format!("cannot create managed runtime stdout capture: {error}"))?;
    let mut stderr = tempfile::tempfile()
        .map_err(|error| format!("cannot create managed runtime stderr capture: {error}"))?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().map_err(|error| {
            format!("cannot clone managed runtime stdout capture: {error}")
        })?))
        .stderr(Stdio::from(stderr.try_clone().map_err(|error| {
            format!("cannot clone managed runtime stderr capture: {error}")
        })?));

    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start managed runtime probe: {error}"))?;
    let started = Instant::now();
    let mut timed_out = false;
    let success = loop {
        match child
            .try_wait()
            .map_err(|error| format!("cannot inspect managed runtime probe: {error}"))?
        {
            Some(status) => break status.success(),
            None if started.elapsed() >= timeout => {
                timed_out = true;
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
            None => thread::sleep(Duration::from_millis(10)),
        }
    };

    stdout
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind managed runtime stdout capture: {error}"))?;
    stderr
        .seek(SeekFrom::Start(0))
        .map_err(|error| format!("cannot rewind managed runtime stderr capture: {error}"))?;
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    stdout
        .take(MANAGED_RUNTIME_PROBE_CAPTURE_BYTES)
        .read_to_end(&mut stdout_bytes)
        .map_err(|error| format!("cannot read managed runtime stdout capture: {error}"))?;
    stderr
        .take(MANAGED_RUNTIME_PROBE_CAPTURE_BYTES)
        .read_to_end(&mut stderr_bytes)
        .map_err(|error| format!("cannot read managed runtime stderr capture: {error}"))?;

    Ok(BoundedOutput {
        success,
        timed_out,
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

fn read_prefix(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MANAGED_RUNTIME_PROBE_CAPTURE_BYTES)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn contains_svg(bytes: &[u8]) -> bool {
    bytes.windows(4).any(|window| window == b"<svg")
}

fn extract_missing_libraries(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    let mut libraries = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.split("error while loading shared libraries:").nth(1)
            && let Some(candidate) = rest.split(':').next()
            && let Some(library) = stable_library_name(candidate)
        {
            libraries.push(library);
        }
        if line.contains("=> not found")
            && let Some(candidate) = line.split("=>").next()
            && let Some(library) = stable_library_name(candidate)
        {
            libraries.push(library);
        }
        if line.contains("cannot open shared object file") {
            for candidate in line
                .split(|character: char| character.is_whitespace() || character == ':')
            {
                if let Some(library) = stable_library_name(candidate) {
                    libraries.push(library);
                }
            }
        }
    }
    libraries.sort();
    libraries.dedup();
    libraries
}

fn stable_library_name(candidate: &str) -> Option<String> {
    let candidate = candidate.trim_matches(|character: char| {
        matches!(character, '`' | '\'' | '"' | ',' | ';' | '(' | ')' | '[' | ']')
    });
    if candidate.len() > 128
        || !candidate.starts_with("lib")
        || !candidate.contains(".so")
        || !candidate
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._+-".contains(character))
    {
        return None;
    }
    Some(candidate.to_owned())
}

#[cfg(target_os = "linux")]
fn missing_linux_libraries(browser: &Path) -> Vec<String> {
    let mut command = Command::new("ldd");
    command.arg(browser).env("LC_ALL", "C");
    let Ok(output) = run_bounded(command, LINUX_LIBRARY_PROBE_TIMEOUT) else {
        return Vec::new();
    };
    let mut libraries = extract_missing_libraries(&output.stdout);
    libraries.extend(extract_missing_libraries(&output.stderr));
    libraries.sort();
    libraries.dedup();
    libraries
}

#[cfg(not(target_os = "linux"))]
fn missing_linux_libraries(_browser: &Path) -> Vec<String> {
    Vec::new()
}

fn validate_absolute_file(label: &str, path: &Path) -> Result<(), String> {
    invalid_path_reason(path, PathKind::File).map_or(Ok(()), |reason| {
        Err(format!("managed bundle {label} is invalid: {reason}"))
    })
}

/// Run a managed renderer alias when the current executable is named `mmdc`,
/// `tex2svg`, or `chafa`. Normal ptymark invocations return `None`.
///
/// The alias is a copy or hard link of the ptymark native binary. It reads a
/// versioned manifest next to the managed bundle and invokes Node directly with
/// a fixed role-specific entrypoint. No shell or batch file is involved.
pub fn run_if_managed_alias() -> Option<Result<i32, String>> {
    let executable = match env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            return Some(Err(format!(
                "cannot resolve managed launcher executable: {error}"
            )));
        }
    };
    let role = ManagedRole::from_executable(&executable)?;
    Some(run_managed_role(role, &executable))
}

fn run_managed_role(role: ManagedRole, executable: &Path) -> Result<i32, String> {
    let bin_root = executable
        .parent()
        .ok_or_else(|| "managed launcher has no parent directory".to_owned())?;
    let bundle_root = bin_root
        .parent()
        .ok_or_else(|| "managed launcher has no bundle root".to_owned())?;
    let manifest_path = bundle_root.join("bundle.toml");
    let manifest = LoadedManifest::read(&manifest_path)?.into_execution_manifest()?;
    let script = manifest.app_root.join(role.script_suffix());
    validate_absolute_file("renderer entrypoint", &script)?;

    fs::create_dir_all(&manifest.cache_root).map_err(|error| {
        format!(
            "cannot create managed renderer cache `{}`: {error}",
            manifest.cache_root.display()
        )
    })?;

    let mut command = Command::new(&manifest.node_path);
    command.arg(&script);
    if role == ManagedRole::Mermaid
        && let Some(config) = manifest.puppeteer_config_path.as_deref()
    {
        command.arg("--puppeteerConfigFile").arg(config);
    }
    command
        .args(env::args_os().skip(1))
        .env("PUPPETEER_CACHE_DIR", &manifest.cache_root);
    if let Some(browser) = manifest.browser_path.as_deref() {
        command.env("PUPPETEER_EXECUTABLE_PATH", browser);
    }
    if manifest.browser_no_sandbox {
        command.env("PTYMARK_BROWSER_NO_SANDBOX", "1");
    }

    let status = command.status().map_err(|error| {
        format!(
            "cannot start managed renderer `{}` with `{}`: {error}",
            script.display(),
            manifest.node_path.display()
        )
    })?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::{
        MANAGED_BUNDLE_SCHEMA_VERSION, ManagedBundleStatus, ManagedRole, ManagedRuntimeStatus,
        extract_missing_libraries, inspect_managed_alias, probe_managed_alias,
        probe_managed_alias_with_timeout,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    #[test]
    fn executable_names_map_to_fixed_roles() {
        assert_eq!(
            ManagedRole::from_executable(Path::new("mmdc")),
            Some(ManagedRole::Mermaid)
        );
        assert_eq!(
            ManagedRole::from_executable(Path::new("tex2svg.exe")),
            Some(ManagedRole::Math)
        );
        assert_eq!(
            ManagedRole::from_executable(Path::new("chafa")),
            Some(ManagedRole::Presenter)
        );
        assert_eq!(ManagedRole::from_executable(Path::new("ptymark")), None);
    }

    #[test]
    fn managed_schema_is_explicit() {
        assert_eq!(MANAGED_BUNDLE_SCHEMA_VERSION, 1);
    }

    #[test]
    fn inspection_and_execution_share_typed_validation() {
        let root = tempfile::tempdir().expect("temp root");
        let bin = root.path().join("bin");
        fs::create_dir_all(&bin).expect("bin");
        let alias = bin.join("mmdc");
        fs::write(&alias, b"alias").expect("alias");
        let manifest = root.path().join("bundle.toml");
        fs::write(
            &manifest,
            format!(
                "schema_version = 99\nnode_path = {:?}\napp_root = {:?}\ncache_root = {:?}\n",
                root.path().join("node"),
                root.path().join("app"),
                root.path().join("cache")
            ),
        )
        .expect("manifest");

        let inspection = inspect_managed_alias(&alias)
            .expect("managed alias")
            .expect("parsed manifest");
        assert!(matches!(
            inspection.status,
            ManagedBundleStatus::IncompatibleSchema {
                found: 99,
                expected: MANAGED_BUNDLE_SCHEMA_VERSION
            }
        ));
        assert!(!inspection.complete);
    }

    #[test]
    fn missing_library_parser_returns_stable_basenames_only() {
        let libraries = extract_missing_libraries(
            b"/home/alice/chrome: error while loading shared libraries: libnspr4.so: cannot open shared object file\n  libnss3.so => not found\nsecret=/home/alice\n",
        );
        assert_eq!(libraries, ["libnspr4.so", "libnss3.so"]);
    }

    #[cfg(unix)]
    #[test]
    fn managed_runtime_probe_executes_all_fixed_role_samples() {
        let body = r#"#!/bin/sh
[ "${TERM:-}" = dumb ] || exit 41
[ -z "${TMUX:-}" ] || exit 42
case "$(basename "$0")" in
  mmdc)
    output=
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --output ]; then output=$2; shift 2; else shift; fi
    done
    printf '<svg xmlns="http://www.w3.org/2000/svg"></svg>\n' > "$output"
    ;;
  tex2svg)
    printf '<svg xmlns="http://www.w3.org/2000/svg"></svg>\n'
    ;;
  chafa)
    printf '#\n'
    ;;
esac
"#;
        for name in ["mmdc", "tex2svg", "chafa"] {
            let (_root, alias) = managed_fixture(name, body);
            let status = probe_managed_alias(&alias)
                .expect("managed alias")
                .expect("runtime probe");
            assert_eq!(status, ManagedRuntimeStatus::Ready, "role={name}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn managed_runtime_probe_reports_only_missing_library_names() {
        let body = r#"#!/bin/sh
printf '/home/alice/private/chrome: error while loading shared libraries: libnspr4.so: cannot open shared object file: No such file or directory\n' >&2
printf 'libnss3.so => not found\nlibnssutil3.so => not found\nlibsmime3.so => not found\n' >&2
exit 127
"#;
        let (_root, alias) = managed_fixture("mmdc", body);
        let status = probe_managed_alias(&alias)
            .expect("managed alias")
            .expect("runtime probe");
        assert_eq!(
            status,
            ManagedRuntimeStatus::MissingLibraries {
                libraries: vec![
                    "libnspr4.so".to_owned(),
                    "libnss3.so".to_owned(),
                    "libnssutil3.so".to_owned(),
                    "libsmime3.so".to_owned(),
                ],
            }
        );
    }

    #[cfg(unix)]
    #[test]
    fn managed_runtime_probe_has_a_hard_deadline() {
        let (_root, alias) = managed_fixture("chafa", "#!/bin/sh\nsleep 2\n");
        let status = probe_managed_alias_with_timeout(&alias, Duration::from_millis(50))
            .expect("managed alias")
            .expect("runtime probe");
        assert_eq!(status, ManagedRuntimeStatus::TimedOut);
    }

    #[cfg(unix)]
    fn managed_fixture(name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("temp root");
        let bin = root.path().join("bin");
        let app = root.path().join("app");
        fs::create_dir_all(&bin).expect("bin");
        fs::create_dir_all(&app).expect("app");
        let node = root.path().join("node");
        let browser = root.path().join("chrome");
        fs::write(&node, b"node").expect("node");
        fs::write(&browser, b"browser").expect("browser");
        let alias = bin.join(name);
        fs::write(&alias, body).expect("alias");
        let mut permissions = fs::metadata(&alias).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&alias, permissions).expect("chmod");
        fs::write(
            root.path().join("bundle.toml"),
            format!(
                "schema_version = 1\nnode_path = {:?}\napp_root = {:?}\ncache_root = {:?}\nbrowser_path = {:?}\n",
                node,
                app,
                root.path().join("cache"),
                browser,
            ),
        )
        .expect("manifest");
        (root, alias)
    }
}
