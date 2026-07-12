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
    browser_no_sandbox: bool,
}

impl ManagedBundleManifest {
    fn load(path: &Path) -> Result<Self, String> {
        let source = fs::read_to_string(path).map_err(|error| {
            format!(
                "cannot read managed bundle manifest `{}`: {error}",
                path.display()
            )
        })?;
        let manifest: Self = toml::from_str(&source).map_err(|error| {
            format!(
                "cannot parse managed bundle manifest `{}`: {error}",
                path.display()
            )
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_version != MANAGED_BUNDLE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported managed bundle schema {}; expected {}",
                self.schema_version, MANAGED_BUNDLE_SCHEMA_VERSION
            ));
        }
        validate_absolute_file("node_path", &self.node_path)?;
        validate_absolute_directory("app_root", &self.app_root)?;
        if !self.cache_root.is_absolute() {
            return Err("managed bundle cache_root must be absolute".to_owned());
        }
        if let Some(path) = self.browser_path.as_deref() {
            validate_absolute_file("browser_path", path)?;
        }
        Ok(())
    }
}

fn validate_absolute_file(label: &str, path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("managed bundle {label} must be absolute"));
    }
    if !path.is_file() {
        return Err(format!(
            "managed bundle {label} does not name a file: `{}`",
            path.display()
        ));
    }
    Ok(())
}

fn validate_absolute_directory(label: &str, path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("managed bundle {label} must be absolute"));
    }
    if !path.is_dir() {
        return Err(format!(
            "managed bundle {label} does not name a directory: `{}`",
            path.display()
        ));
    }
    Ok(())
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
    let manifest = ManagedBundleManifest::load(&manifest_path)?;
    let script = manifest.app_root.join(role.script_suffix());
    validate_absolute_file("renderer entrypoint", &script)?;

    fs::create_dir_all(&manifest.cache_root).map_err(|error| {
        format!(
            "cannot create managed renderer cache `{}`: {error}",
            manifest.cache_root.display()
        )
    })?;

    let mut command = Command::new(&manifest.node_path);
    command
        .arg(&script)
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
    use super::{MANAGED_BUNDLE_SCHEMA_VERSION, ManagedRole};
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
}
