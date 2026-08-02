use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MANAGED_BUNDLE_SCHEMA_VERSION: u32 = 1;

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
        ("cache_root", manifest.cache_root.as_path(), PathKind::Absolute),
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

fn validate_absolute_file(label: &str, path: &Path) -> Result<(), String> {
    invalid_path_reason(path, PathKind::File)
        .map_or(Ok(()), |reason| Err(format!("managed bundle {label} is invalid: {reason}")))
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
        MANAGED_BUNDLE_SCHEMA_VERSION, ManagedBundleStatus, ManagedRole, inspect_managed_alias,
    };
    use std::fs;
    use std::path::Path;

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
}
