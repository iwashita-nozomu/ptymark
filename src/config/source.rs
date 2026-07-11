use super::ConfigError;
use serde::Serialize;
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigOrigin {
    User,
    Environment,
    Explicit,
    Project,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigTrust {
    UserOwned,
    ExplicitlySelected,
    TrustedProject,
    UntrustedProject,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigSource {
    pub origin: ConfigOrigin,
    pub trust: ConfigTrust,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConfigEnvironment {
    pub home: Option<PathBuf>,
    pub xdg_config_home: Option<PathBuf>,
    pub app_data: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub profile: Option<String>,
    pub no_config: bool,
}

impl ConfigEnvironment {
    pub fn from_process() -> Self {
        Self {
            home: env::var_os("HOME").map(PathBuf::from),
            xdg_config_home: env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            app_data: env::var_os("APPDATA").map(PathBuf::from),
            config_path: env::var_os("PTYMARK_CONFIG").map(PathBuf::from),
            profile: env::var("PTYMARK_PROFILE")
                .ok()
                .filter(|value| !value.is_empty()),
            no_config: env::var("PTYMARK_NO_CONFIG")
                .ok()
                .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigRequest {
    pub explicit_path: Option<PathBuf>,
    pub profile: Option<String>,
    pub no_config: bool,
    pub working_directory: PathBuf,
}

impl Default for ConfigRequest {
    fn default() -> Self {
        Self {
            explicit_path: None,
            profile: None,
            no_config: false,
            working_directory: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

pub trait ConfigLocator: Send + Sync {
    fn locate(
        &self,
        request: &ConfigRequest,
        environment: &ConfigEnvironment,
    ) -> Result<Vec<ConfigSource>, ConfigError>;

    fn candidate_paths(
        &self,
        request: &ConfigRequest,
        environment: &ConfigEnvironment,
    ) -> Vec<ConfigSource>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FilesystemConfigLocator;

impl FilesystemConfigLocator {
    fn user_path(environment: &ConfigEnvironment) -> Option<PathBuf> {
        if let Some(root) = &environment.xdg_config_home {
            return Some(root.join("ptymark").join("config.toml"));
        }

        if cfg!(target_os = "macos") {
            return environment.home.as_ref().map(|home| {
                home.join("Library")
                    .join("Application Support")
                    .join("ptymark")
                    .join("config.toml")
            });
        }

        if cfg!(windows) {
            return environment
                .app_data
                .as_ref()
                .map(|root| root.join("ptymark").join("config.toml"));
        }

        environment
            .home
            .as_ref()
            .map(|home| home.join(".config").join("ptymark").join("config.toml"))
    }

    fn project_candidate(request: &ConfigRequest) -> PathBuf {
        request.working_directory.join(".ptymark.toml")
    }

    fn environment_source(environment: &ConfigEnvironment) -> Option<ConfigSource> {
        environment.config_path.as_ref().map(|path| ConfigSource {
            origin: ConfigOrigin::Environment,
            trust: ConfigTrust::ExplicitlySelected,
            path: path.clone(),
        })
    }

    fn explicit_source(request: &ConfigRequest) -> Option<ConfigSource> {
        request.explicit_path.as_ref().map(|path| ConfigSource {
            origin: ConfigOrigin::Explicit,
            trust: ConfigTrust::ExplicitlySelected,
            path: path.clone(),
        })
    }

    fn push_existing_source(
        sources: &mut Vec<ConfigSource>,
        seen: &mut HashSet<PathBuf>,
        source: ConfigSource,
    ) -> Result<(), ConfigError> {
        if !source.path.is_file() {
            let label = match source.origin {
                ConfigOrigin::Environment => "PTYMARK_CONFIG file",
                ConfigOrigin::Explicit => "explicit configuration file",
                ConfigOrigin::User | ConfigOrigin::Project => "configuration file",
            };
            return Err(ConfigError::io(
                Some(source.path.clone()),
                format!("{label} does not exist"),
            ));
        }
        if seen.insert(canonical_or_original(&source.path)) {
            sources.push(source);
        }
        Ok(())
    }
}

impl ConfigLocator for FilesystemConfigLocator {
    fn locate(
        &self,
        request: &ConfigRequest,
        environment: &ConfigEnvironment,
    ) -> Result<Vec<ConfigSource>, ConfigError> {
        if request.no_config {
            return Ok(Vec::new());
        }

        let explicit = Self::explicit_source(request);
        let ambient_enabled = !environment.no_config;
        let mut sources = Vec::new();
        let mut seen = HashSet::new();

        if ambient_enabled {
            if let Some(path) = Self::user_path(environment)
                && path.is_file()
            {
                seen.insert(canonical_or_original(&path));
                sources.push(ConfigSource {
                    origin: ConfigOrigin::User,
                    trust: ConfigTrust::UserOwned,
                    path,
                });
            }
            if let Some(source) = Self::environment_source(environment) {
                Self::push_existing_source(&mut sources, &mut seen, source)?;
            }
        }

        // A command-line --config is the highest-priority file source and deliberately overrides
        // ambient PTYMARK_NO_CONFIG. This lets a launcher suppress ambient state while selecting one
        // reviewed file explicitly. CLI --no-config still disables every external file source.
        if let Some(source) = explicit {
            Self::push_existing_source(&mut sources, &mut seen, source)?;
        }

        // Project configuration is deliberately not auto-loaded in v1. The candidate is exposed by
        // `config paths` so a future trust store can approve it without changing discovery APIs.
        Ok(sources)
    }

    fn candidate_paths(
        &self,
        request: &ConfigRequest,
        environment: &ConfigEnvironment,
    ) -> Vec<ConfigSource> {
        let mut candidates = Vec::new();
        if let Some(path) = Self::user_path(environment) {
            candidates.push(ConfigSource {
                origin: ConfigOrigin::User,
                trust: ConfigTrust::UserOwned,
                path,
            });
        }
        if let Some(source) = Self::environment_source(environment) {
            candidates.push(source);
        }
        if let Some(source) = Self::explicit_source(request) {
            candidates.push(source);
        }
        candidates.push(ConfigSource {
            origin: ConfigOrigin::Project,
            trust: ConfigTrust::UntrustedProject,
            path: Self::project_candidate(request),
        });
        candidates
    }
}

pub fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
