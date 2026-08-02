use directories::ProjectDirs;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PlatformPaths {
    pub(crate) config_file: PathBuf,
    pub(crate) install_state_file: PathBuf,
    pub(crate) data_dir: PathBuf,
    pub(crate) cache_dir: PathBuf,
}

impl PlatformPaths {
    pub(crate) fn discover() -> Result<Self, String> {
        let project = ProjectDirs::from("", "", "ptymark")
            .ok_or_else(|| "cannot determine platform directories for ptymark".to_owned())?;

        let config_file = env_path("PTYMARK_CONFIG")
            .unwrap_or_else(|| project.config_dir().join("config.toml"));
        let install_state_file = env_path("PTYMARK_INSTALL_STATE").unwrap_or_else(|| {
            project
                .state_dir()
                .unwrap_or_else(|| project.data_local_dir())
                .join("install.toml")
        });
        let data_dir = env_path("PTYMARK_DATA_HOME")
            .unwrap_or_else(|| project.data_local_dir().to_path_buf());
        let cache_dir = env_path("PTYMARK_CACHE_HOME")
            .unwrap_or_else(|| project.cache_dir().to_path_buf());

        Ok(Self {
            config_file,
            install_state_file,
            data_dir,
            cache_dir,
        })
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from)
}

pub(crate) fn resolve_executable(configured: &Path) -> Result<PathBuf, String> {
    reject_windows_shell_wrapper(configured)?;
    if configured.as_os_str().is_empty() {
        return Err("configured executable cannot be empty".to_owned());
    }
    if !configured.is_absolute() && configured.components().count() != 1 {
        return Err(format!(
            "configured executable `{}` must be absolute or a bare name",
            configured.display()
        ));
    }

    which::which(configured).map_err(|error| {
        if configured.is_absolute() {
            format!(
                "configured executable `{}` does not exist or is not executable: {error}",
                configured.display()
            )
        } else {
            format!(
                "executable `{}` was not found in PATH: {error}",
                configured.display()
            )
        }
    })
}

#[cfg(windows)]
fn reject_windows_shell_wrapper(path: &Path) -> Result<(), String> {
    let is_batch = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
        });
    if is_batch {
        return Err(format!(
            "configured renderer `{}` is a shell wrapper; select a native .exe/.com or the ptymark-managed alias",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn reject_windows_shell_wrapper(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_executable;
    use std::path::Path;

    #[test]
    fn relative_paths_with_directories_are_rejected() {
        let error = resolve_executable(Path::new("tools/renderer"))
            .expect_err("relative paths with directories must fail");
        assert!(error.contains("absolute or a bare name"));
    }
}
