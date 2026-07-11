use super::model::{
    CONFIG_SCHEMA_VERSION, CacheBackend, CacheConfig, CachePolicyConfig, ConfigFile,
    DetectionConfig, DetectionMode, DetectionPolicy, DiagnosticFormat, DiagnosticLevel,
    DiagnosticSink, DiagnosticsConfig, DiagnosticsPolicy, EngineSelectionConfig,
    EngineSelectionPolicy, FallbackPolicy, PresentationConfig, PresentationMode,
    PresentationPolicy, ProfileConfig, RenderConfig, RenderOrdering, RenderPolicy,
    RendererBundleConfig, ResolvedConfig, RuntimeConfig, SessionMode, UnsupportedPresentation,
};
use super::source::{
    ConfigEnvironment, ConfigLocator, ConfigRequest, ConfigSource, FilesystemConfigLocator,
    canonical_or_original,
};
use super::{ConfigError, ConfigProvenance, LoadedConfig};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;

const BUILTIN_ENGINES: &[&str] = &[
    "mermaid-worker",
    "mermaid-cli",
    "mathjax-worker",
    "katex",
    "typst",
    "source",
    "preview",
];
const SEMANTIC_KINDS: &[&str] = &["mermaid", "math"];
const ARTIFACT_TYPES: &[&str] = &[
    "image/svg+xml",
    "image/png",
    "application/mathml+xml",
    "text/plain",
];
const IMAGE_PROTOCOLS: &[&str] = &["kitty", "iterm2", "sixel"];

pub struct ConfigManager<L = FilesystemConfigLocator> {
    locator: L,
}

impl Default for ConfigManager<FilesystemConfigLocator> {
    fn default() -> Self {
        Self {
            locator: FilesystemConfigLocator,
        }
    }
}

impl<L: ConfigLocator> ConfigManager<L> {
    pub const fn new(locator: L) -> Self {
        Self { locator }
    }

    pub fn candidate_paths(
        &self,
        request: &ConfigRequest,
        environment: &ConfigEnvironment,
    ) -> Vec<ConfigSource> {
        self.locator.candidate_paths(request, environment)
    }

    pub fn load_from_process(&self, request: ConfigRequest) -> Result<LoadedConfig, ConfigError> {
        self.load(request, ConfigEnvironment::from_process())
    }

    pub fn load(
        &self,
        request: ConfigRequest,
        environment: ConfigEnvironment,
    ) -> Result<LoadedConfig, ConfigError> {
        let sources = self.locator.locate(&request, &environment)?;
        let mut merged = builtin_config();

        for source in &sources {
            let text = fs::read_to_string(&source.path).map_err(|error| {
                ConfigError::io(
                    Some(source.path.clone()),
                    format!("cannot read configuration: {error}"),
                )
            })?;
            let document: ConfigFile = toml::from_str(&text).map_err(|error| {
                ConfigError::parse(Some(source.path.clone()), error.to_string())
            })?;
            validate_schema(&document, Some(source.path.clone()))?;
            merge_file(&mut merged, document);
        }

        let profile_name = request
            .profile
            .or(environment.profile)
            .or_else(|| merged.default_profile.clone())
            .unwrap_or_else(|| "interactive".to_owned());
        let profile = resolve_profile(&profile_name, &merged.profiles)?;
        let config = materialize(profile_name.clone(), profile, merged)?;
        validate_resolved(&config)?;

        Ok(LoadedConfig {
            config,
            provenance: ConfigProvenance {
                selected_profile: profile_name,
                sources: sources
                    .into_iter()
                    .map(|mut source| {
                        source.path = canonical_or_original(&source.path);
                        source
                    })
                    .collect(),
            },
        })
    }
}

fn validate_schema(document: &ConfigFile, path: Option<std::path::PathBuf>) -> Result<(), ConfigError> {
    if document.schema_version != CONFIG_SCHEMA_VERSION {
        return Err(ConfigError::schema(
            path,
            format!(
                "unsupported schema_version {}; expected {}",
                document.schema_version, CONFIG_SCHEMA_VERSION
            ),
        ));
    }
    Ok(())
}

fn builtin_config() -> ConfigFile {
    let mut profiles = BTreeMap::new();
    profiles.insert("interactive".to_owned(), ProfileConfig::default());

    let source = ProfileConfig {
        extends: Some("interactive".to_owned()),
        mode: Some(SessionMode::Source),
        presentation: PresentationConfig {
            mode: Some(PresentationMode::Source),
            image_protocols: Some(Vec::new()),
            ..PresentationConfig::default()
        },
        cache: CacheConfig {
            backend: Some(CacheBackend::None),
            ..CacheConfig::default()
        },
        ..ProfileConfig::default()
    };
    profiles.insert("source".to_owned(), source);

    let private = ProfileConfig {
        extends: Some("interactive".to_owned()),
        cache: CacheConfig {
            backend: Some(CacheBackend::None),
            private: Some(true),
            ..CacheConfig::default()
        },
        diagnostics: DiagnosticsConfig {
            include_source: Some(false),
            ..DiagnosticsConfig::default()
        },
        ..ProfileConfig::default()
    };
    profiles.insert("private".to_owned(), private);

    let ci = ProfileConfig {
        extends: Some("interactive".to_owned()),
        presentation: PresentationConfig {
            mode: Some(PresentationMode::Source),
            image_protocols: Some(Vec::new()),
            ..PresentationConfig::default()
        },
        render: RenderConfig {
            soft_latency_budget_ms: Some(1_000),
            hard_timeout_ms: Some(10_000),
            prewarm: Some(false),
            ..RenderConfig::default()
        },
        cache: CacheConfig {
            backend: Some(CacheBackend::None),
            ..CacheConfig::default()
        },
        ..ProfileConfig::default()
    };
    profiles.insert("ci".to_owned(), ci);

    ConfigFile {
        schema_version: CONFIG_SCHEMA_VERSION,
        default_profile: Some("interactive".to_owned()),
        profiles,
        engines: BTreeMap::new(),
        runtimes: BTreeMap::new(),
        renderer_bundle: RendererBundleConfig {
            path: None,
            require_lock_match: Some(true),
        },
        diagnostics: DiagnosticsConfig::default(),
    }
}

fn merge_file(base: &mut ConfigFile, overlay: ConfigFile) {
    if overlay.default_profile.is_some() {
        base.default_profile = overlay.default_profile;
    }
    for (name, profile) in overlay.profiles {
        merge_profile(base.profiles.entry(name).or_default(), profile);
    }
    base.engines.extend(overlay.engines);
    base.runtimes.extend(overlay.runtimes);
    merge_renderer_bundle(&mut base.renderer_bundle, overlay.renderer_bundle);
    merge_diagnostics(&mut base.diagnostics, overlay.diagnostics);
}

fn merge_profile(base: &mut ProfileConfig, overlay: ProfileConfig) {
    replace_if_some(&mut base.extends, overlay.extends);
    replace_if_some(&mut base.mode, overlay.mode);
    replace_if_some(&mut base.strict, overlay.strict);
    replace_if_some(&mut base.fallback, overlay.fallback);
    merge_detection(&mut base.detection, overlay.detection);
    merge_render(&mut base.render, overlay.render);
    merge_presentation(&mut base.presentation, overlay.presentation);
    merge_cache(&mut base.cache, overlay.cache);
    merge_diagnostics(&mut base.diagnostics, overlay.diagnostics);
    for (kind, selection) in overlay.engines {
        merge_engine_selection(base.engines.entry(kind).or_default(), selection);
    }
}

fn replace_if_some<T>(base: &mut Option<T>, overlay: Option<T>) {
    if overlay.is_some() {
        *base = overlay;
    }
}

fn merge_detection(base: &mut DetectionConfig, overlay: DetectionConfig) {
    replace_if_some(&mut base.mode, overlay.mode);
    replace_if_some(&mut base.mermaid, overlay.mermaid);
    replace_if_some(&mut base.block_math, overlay.block_math);
    replace_if_some(&mut base.max_buffer_bytes, overlay.max_buffer_bytes);
    replace_if_some(&mut base.max_line_bytes, overlay.max_line_bytes);
    replace_if_some(&mut base.fences.mermaid, overlay.fences.mermaid);
    replace_if_some(&mut base.fences.math, overlay.fences.math);
}

fn merge_engine_selection(base: &mut EngineSelectionConfig, overlay: EngineSelectionConfig) {
    replace_if_some(&mut base.candidates, overlay.candidates);
    replace_if_some(
        &mut base.preferred_artifacts,
        overlay.preferred_artifacts,
    );
}

fn merge_render(base: &mut RenderConfig, overlay: RenderConfig) {
    replace_if_some(
        &mut base.soft_latency_budget_ms,
        overlay.soft_latency_budget_ms,
    );
    replace_if_some(&mut base.hard_timeout_ms, overlay.hard_timeout_ms);
    replace_if_some(&mut base.max_in_flight, overlay.max_in_flight);
    replace_if_some(&mut base.ordering, overlay.ordering);
    replace_if_some(&mut base.prewarm, overlay.prewarm);
    replace_if_some(&mut base.worker_idle_ms, overlay.worker_idle_ms);
    replace_if_some(
        &mut base.worker_max_requests,
        overlay.worker_max_requests,
    );
}

fn merge_presentation(base: &mut PresentationConfig, overlay: PresentationConfig) {
    replace_if_some(&mut base.mode, overlay.mode);
    replace_if_some(&mut base.prefer, overlay.prefer);
    replace_if_some(&mut base.image_protocols, overlay.image_protocols);
    replace_if_some(&mut base.unsupported, overlay.unsupported);
    replace_if_some(
        &mut base.transparent_background,
        overlay.transparent_background,
    );
    replace_if_some(&mut base.max_columns, overlay.max_columns);
    replace_if_some(&mut base.max_rows, overlay.max_rows);
    replace_if_some(
        &mut base.preserve_aspect_ratio,
        overlay.preserve_aspect_ratio,
    );
}

fn merge_cache(base: &mut CacheConfig, overlay: CacheConfig) {
    replace_if_some(&mut base.backend, overlay.backend);
    replace_if_some(&mut base.max_entries, overlay.max_entries);
    replace_if_some(&mut base.max_bytes, overlay.max_bytes);
    replace_if_some(&mut base.ttl_seconds, overlay.ttl_seconds);
    replace_if_some(&mut base.path, overlay.path);
    replace_if_some(&mut base.private, overlay.private);
}

fn merge_diagnostics(base: &mut DiagnosticsConfig, overlay: DiagnosticsConfig) {
    replace_if_some(&mut base.level, overlay.level);
    replace_if_some(&mut base.format, overlay.format);
    replace_if_some(&mut base.sink, overlay.sink);
    replace_if_some(&mut base.path, overlay.path);
    replace_if_some(&mut base.include_source, overlay.include_source);
    replace_if_some(&mut base.metrics, overlay.metrics);
}

fn merge_renderer_bundle(base: &mut RendererBundleConfig, overlay: RendererBundleConfig) {
    replace_if_some(&mut base.path, overlay.path);
    replace_if_some(
        &mut base.require_lock_match,
        overlay.require_lock_match,
    );
}

fn resolve_profile(
    name: &str,
    profiles: &BTreeMap<String, ProfileConfig>,
) -> Result<ProfileConfig, ConfigError> {
    fn visit(
        name: &str,
        profiles: &BTreeMap<String, ProfileConfig>,
        stack: &mut Vec<String>,
        memo: &mut BTreeMap<String, ProfileConfig>,
    ) -> Result<ProfileConfig, ConfigError> {
        if let Some(profile) = memo.get(name) {
            return Ok(profile.clone());
        }
        if let Some(index) = stack.iter().position(|item| item == name) {
            let mut cycle = stack[index..].to_vec();
            cycle.push(name.to_owned());
            return Err(ConfigError::profile(format!(
                "profile inheritance cycle: {}",
                cycle.join(" -> ")
            )));
        }
        let raw = profiles
            .get(name)
            .cloned()
            .ok_or_else(|| ConfigError::profile(format!("unknown profile `{name}`")))?;
        stack.push(name.to_owned());
        let mut resolved = if let Some(parent) = raw.extends.as_deref() {
            visit(parent, profiles, stack, memo)?
        } else {
            ProfileConfig::default()
        };
        merge_profile(&mut resolved, raw);
        resolved.extends = None;
        stack.pop();
        memo.insert(name.to_owned(), resolved.clone());
        Ok(resolved)
    }

    visit(name, profiles, &mut Vec::new(), &mut BTreeMap::new())
}

fn materialize(
    profile_name: String,
    profile: ProfileConfig,
    file: ConfigFile,
) -> Result<ResolvedConfig, ConfigError> {
    if let (Some(strict), Some(fallback)) = (profile.strict, profile.fallback)
        && strict != (fallback == FallbackPolicy::Error)
    {
        return Err(ConfigError::validation(
            "profile `strict` and `fallback` specify conflicting behavior",
        ));
    }

    let fallback = profile.fallback.unwrap_or_else(|| {
        if profile.strict.unwrap_or(false) {
            FallbackPolicy::Error
        } else {
            FallbackPolicy::Source
        }
    });

    let detection = DetectionPolicy {
        mode: profile
            .detection
            .mode
            .unwrap_or(DetectionMode::ExplicitBlocks),
        mermaid: profile.detection.mermaid.unwrap_or(true),
        block_math: profile.detection.block_math.unwrap_or(true),
        max_buffer_bytes: profile.detection.max_buffer_bytes.unwrap_or(1024 * 1024),
        max_line_bytes: profile.detection.max_line_bytes.unwrap_or(64 * 1024),
        mermaid_fences: profile
            .detection
            .fences
            .mermaid
            .unwrap_or_else(|| vec!["mermaid".to_owned()]),
        math_fences: profile.detection.fences.math.unwrap_or_else(|| {
            vec!["math".to_owned(), "latex".to_owned(), "tex".to_owned()]
        }),
    };

    let mut engines = BTreeMap::new();
    engines.insert(
        "mermaid".to_owned(),
        resolve_engine_selection(
            profile.engines.get("mermaid"),
            &["mermaid-worker", "mermaid-cli", "source"],
            &["image/svg+xml", "text/plain"],
        ),
    );
    engines.insert(
        "math".to_owned(),
        resolve_engine_selection(
            profile.engines.get("math"),
            &["mathjax-worker", "katex", "source"],
            &["image/svg+xml", "application/mathml+xml", "text/plain"],
        ),
    );
    for (kind, selection) in &profile.engines {
        if !engines.contains_key(kind) {
            engines.insert(
                kind.clone(),
                resolve_engine_selection(Some(selection), &[], &["text/plain"]),
            );
        }
    }

    let render = RenderPolicy {
        soft_latency_budget_ms: profile.render.soft_latency_budget_ms.unwrap_or(250),
        hard_timeout_ms: profile.render.hard_timeout_ms.unwrap_or(1_500),
        max_in_flight: profile.render.max_in_flight.unwrap_or(1),
        ordering: profile.render.ordering.unwrap_or(RenderOrdering::Strict),
        prewarm: profile.render.prewarm.unwrap_or(true),
        worker_idle_ms: profile.render.worker_idle_ms.unwrap_or(300_000),
        worker_max_requests: profile.render.worker_max_requests.unwrap_or(1_000),
    };

    let presentation = PresentationPolicy {
        mode: profile.presentation.mode.unwrap_or(PresentationMode::Auto),
        prefer: profile
            .presentation
            .prefer
            .unwrap_or_else(|| vec!["image/svg+xml".to_owned(), "text/plain".to_owned()]),
        image_protocols: profile.presentation.image_protocols.unwrap_or_else(|| {
            vec!["kitty".to_owned(), "iterm2".to_owned(), "sixel".to_owned()]
        }),
        unsupported: profile
            .presentation
            .unsupported
            .unwrap_or(UnsupportedPresentation::Source),
        transparent_background: profile
            .presentation
            .transparent_background
            .unwrap_or(true),
        max_columns: profile.presentation.max_columns.unwrap_or(120),
        max_rows: profile.presentation.max_rows.unwrap_or(40),
        preserve_aspect_ratio: profile
            .presentation
            .preserve_aspect_ratio
            .unwrap_or(true),
    };

    let private = profile.cache.private.unwrap_or(false);
    let cache = CachePolicyConfig {
        backend: if private {
            CacheBackend::None
        } else {
            profile.cache.backend.unwrap_or(CacheBackend::Memory)
        },
        max_entries: profile.cache.max_entries.unwrap_or(128),
        max_bytes: profile.cache.max_bytes.unwrap_or(32 * 1024 * 1024),
        ttl_seconds: profile.cache.ttl_seconds,
        path: profile.cache.path,
        private,
    };

    let mut diagnostics_config = file.diagnostics;
    merge_diagnostics(&mut diagnostics_config, profile.diagnostics);
    let diagnostics = DiagnosticsPolicy {
        level: diagnostics_config.level.unwrap_or(DiagnosticLevel::Warn),
        format: diagnostics_config.format.unwrap_or(DiagnosticFormat::Text),
        sink: diagnostics_config.sink.unwrap_or(DiagnosticSink::Stderr),
        path: diagnostics_config.path,
        include_source: if private {
            false
        } else {
            diagnostics_config.include_source.unwrap_or(false)
        },
        metrics: diagnostics_config.metrics.unwrap_or(true),
    };

    let renderer_bundle = RendererBundleConfig {
        path: file.renderer_bundle.path,
        require_lock_match: Some(file.renderer_bundle.require_lock_match.unwrap_or(true)),
    };

    Ok(ResolvedConfig {
        schema_version: CONFIG_SCHEMA_VERSION,
        profile: profile_name,
        mode: profile.mode.unwrap_or(SessionMode::Transform),
        fallback,
        detection,
        engines,
        render,
        presentation,
        cache,
        diagnostics,
        external_engines: file.engines,
        runtimes: normalize_runtimes(file.runtimes),
        renderer_bundle,
    })
}

fn normalize_runtimes(
    mut runtimes: BTreeMap<String, RuntimeConfig>,
) -> BTreeMap<String, RuntimeConfig> {
    runtimes.entry("node".to_owned()).or_insert(RuntimeConfig {
        program: None,
        required_version: Some(">=24.18.0 <25".to_owned()),
        args: Vec::new(),
    });
    runtimes
        .entry("chromium".to_owned())
        .or_insert_with(RuntimeConfig::default);
    runtimes
}

fn resolve_engine_selection(
    selection: Option<&EngineSelectionConfig>,
    default_candidates: &[&str],
    default_artifacts: &[&str],
) -> EngineSelectionPolicy {
    let mut candidates = selection
        .and_then(|value| value.candidates.clone())
        .unwrap_or_else(|| default_candidates.iter().map(ToString::to_string).collect());
    if !candidates.iter().any(|candidate| candidate == "source") {
        candidates.push("source".to_owned());
    }
    EngineSelectionPolicy {
        candidates,
        preferred_artifacts: selection
            .and_then(|value| value.preferred_artifacts.clone())
            .unwrap_or_else(|| default_artifacts.iter().map(ToString::to_string).collect()),
    }
}

fn validate_resolved(config: &ResolvedConfig) -> Result<(), ConfigError> {
    if config.profile.trim().is_empty() {
        return Err(ConfigError::validation("profile name cannot be empty"));
    }
    if config.detection.max_buffer_bytes == 0 || config.detection.max_line_bytes == 0 {
        return Err(ConfigError::validation(
            "detection byte limits must be greater than zero",
        ));
    }
    if config.detection.max_line_bytes > config.detection.max_buffer_bytes {
        return Err(ConfigError::validation(
            "detection.max_line_bytes cannot exceed detection.max_buffer_bytes",
        ));
    }
    validate_unique_nonempty("detection.fences.mermaid", &config.detection.mermaid_fences)?;
    validate_unique_nonempty("detection.fences.math", &config.detection.math_fences)?;

    if config.render.soft_latency_budget_ms > config.render.hard_timeout_ms {
        return Err(ConfigError::validation(
            "render.soft_latency_budget_ms cannot exceed render.hard_timeout_ms",
        ));
    }
    if config.render.hard_timeout_ms == 0 || config.render.max_in_flight == 0 {
        return Err(ConfigError::validation(
            "render hard timeout and max_in_flight must be greater than zero",
        ));
    }

    if config.cache.backend != CacheBackend::None
        && (config.cache.max_entries == 0 || config.cache.max_bytes == 0)
    {
        return Err(ConfigError::validation(
            "enabled cache backends require non-zero max_entries and max_bytes",
        ));
    }
    if config.cache.private && config.cache.backend != CacheBackend::None {
        return Err(ConfigError::validation(
            "private cache mode must resolve to backend = none",
        ));
    }
    if config.cache.private && config.diagnostics.include_source {
        return Err(ConfigError::validation(
            "private mode cannot include source in diagnostics",
        ));
    }

    for (kind, selection) in &config.engines {
        if !SEMANTIC_KINDS.contains(&kind.as_str()) {
            return Err(ConfigError::validation(format!(
                "unknown semantic kind `{kind}` in engine policy"
            )));
        }
        if selection.candidates.is_empty() {
            return Err(ConfigError::validation(format!(
                "engine policy `{kind}` has no candidates"
            )));
        }
        for candidate in &selection.candidates {
            if !BUILTIN_ENGINES.contains(&candidate.as_str())
                && !config.external_engines.contains_key(candidate)
            {
                return Err(ConfigError::validation(format!(
                    "engine candidate `{candidate}` is neither built-in nor configured"
                )));
            }
        }
        for artifact in &selection.preferred_artifacts {
            if !ARTIFACT_TYPES.contains(&artifact.as_str()) {
                return Err(ConfigError::validation(format!(
                    "unsupported artifact type `{artifact}` in `{kind}` engine policy"
                )));
            }
        }
    }

    for protocol in &config.presentation.image_protocols {
        if !IMAGE_PROTOCOLS.contains(&protocol.as_str()) {
            return Err(ConfigError::validation(format!(
                "unknown image protocol `{protocol}`"
            )));
        }
    }
    for artifact in &config.presentation.prefer {
        if !ARTIFACT_TYPES.contains(&artifact.as_str()) {
            return Err(ConfigError::validation(format!(
                "unsupported presentation artifact type `{artifact}`"
            )));
        }
    }
    if config.presentation.max_columns == 0 || config.presentation.max_rows == 0 {
        return Err(ConfigError::validation(
            "presentation dimensions must be greater than zero",
        ));
    }

    for (name, engine) in &config.external_engines {
        if name.trim().is_empty() || engine.version.trim().is_empty() {
            return Err(ConfigError::validation(
                "external engine names and versions cannot be empty",
            ));
        }
        if engine.program.as_os_str().is_empty() {
            return Err(ConfigError::validation(format!(
                "external engine `{name}` has an empty program path"
            )));
        }
        if engine.semantic_kinds.is_empty() || engine.artifact_types.is_empty() {
            return Err(ConfigError::validation(format!(
                "external engine `{name}` must declare semantic_kinds and artifact_types"
            )));
        }
        for kind in &engine.semantic_kinds {
            if !SEMANTIC_KINDS.contains(&kind.as_str()) {
                return Err(ConfigError::validation(format!(
                    "external engine `{name}` declares unknown semantic kind `{kind}`"
                )));
            }
        }
        for artifact in &engine.artifact_types {
            if !ARTIFACT_TYPES.contains(&artifact.as_str()) {
                return Err(ConfigError::validation(format!(
                    "external engine `{name}` declares unsupported artifact type `{artifact}`"
                )));
            }
        }
        if engine.timeout_ms == 0
            || engine.max_stdout_bytes == 0
            || engine.max_stderr_bytes == 0
        {
            return Err(ConfigError::validation(format!(
                "external engine `{name}` requires non-zero process limits"
            )));
        }
        for key in engine
            .environment
            .keys()
            .chain(engine.inherit_environment.iter())
        {
            if key.is_empty() || key.contains('=') {
                return Err(ConfigError::validation(format!(
                    "external engine `{name}` contains an invalid environment key"
                )));
            }
        }
    }

    if matches!(config.diagnostics.sink, DiagnosticSink::File | DiagnosticSink::Both)
        && config.diagnostics.path.is_none()
    {
        return Err(ConfigError::validation(
            "diagnostics.path is required when sink includes file",
        ));
    }

    Ok(())
}

fn validate_unique_nonempty(label: &str, values: &[String]) -> Result<(), ConfigError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if value.trim().is_empty() {
            return Err(ConfigError::validation(format!(
                "{label} cannot contain an empty value"
            )));
        }
        if !seen.insert(value) {
            return Err(ConfigError::validation(format!(
                "{label} contains duplicate value `{value}`"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ConfigManager;
    use crate::config::{
        CacheBackend, ConfigEnvironment, ConfigRequest, ConfigSource, ConfigTrust,
        FilesystemConfigLocator, SessionMode,
    };
    use crate::config::{ConfigError, ConfigLocator, ConfigOrigin};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Clone, Debug)]
    struct FixedLocator {
        sources: Vec<ConfigSource>,
    }

    impl ConfigLocator for FixedLocator {
        fn locate(
            &self,
            _request: &ConfigRequest,
            _environment: &ConfigEnvironment,
        ) -> Result<Vec<ConfigSource>, ConfigError> {
            Ok(self.sources.clone())
        }

        fn candidate_paths(
            &self,
            _request: &ConfigRequest,
            _environment: &ConfigEnvironment,
        ) -> Vec<ConfigSource> {
            self.sources.clone()
        }
    }

    fn temp_file(name: &str, content: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("ptymark-config-{nonce}"));
        fs::create_dir_all(&root).expect("create temp root");
        let path = root.join(name);
        fs::write(&path, content).expect("write config");
        path
    }

    #[test]
    fn builtins_resolve_without_touching_the_filesystem() {
        let loaded = ConfigManager::default()
            .load(ConfigRequest::default(), ConfigEnvironment::default())
            .expect("builtins");
        assert_eq!(loaded.config.profile, "interactive");
        assert_eq!(loaded.config.mode, SessionMode::Transform);
        assert_eq!(loaded.config.cache.backend, CacheBackend::Memory);
        assert!(loaded.provenance.sources.is_empty());
    }

    #[test]
    fn profile_inheritance_and_private_mode_are_resolved_once() {
        let path = temp_file(
            "config.toml",
            r#"
schema_version = 1
default_profile = "quiet"

[profiles.quiet]
extends = "private"

[profiles.quiet.detection]
max_buffer_bytes = 4096
max_line_bytes = 1024
"#,
        );
        let manager = ConfigManager::new(FixedLocator {
            sources: vec![ConfigSource {
                origin: ConfigOrigin::Explicit,
                trust: ConfigTrust::ExplicitlySelected,
                path,
            }],
        });
        let loaded = manager
            .load(ConfigRequest::default(), ConfigEnvironment::default())
            .expect("resolved config");
        assert_eq!(loaded.config.profile, "quiet");
        assert_eq!(loaded.config.cache.backend, CacheBackend::None);
        assert!(loaded.config.cache.private);
        assert!(!loaded.config.diagnostics.include_source);
        assert_eq!(loaded.config.detection.max_buffer_bytes, 4096);
    }

    #[test]
    fn unknown_keys_and_profile_cycles_fail_before_runtime_construction() {
        let unknown = temp_file(
            "unknown.toml",
            "schema_version = 1\nunknown_key = true\n",
        );
        let manager = ConfigManager::new(FixedLocator {
            sources: vec![ConfigSource {
                origin: ConfigOrigin::Explicit,
                trust: ConfigTrust::ExplicitlySelected,
                path: unknown,
            }],
        });
        assert!(manager
            .load(ConfigRequest::default(), ConfigEnvironment::default())
            .expect_err("unknown key must fail")
            .to_string()
            .contains("unknown field"));

        let cycle = temp_file(
            "cycle.toml",
            r#"
schema_version = 1
default_profile = "a"
[profiles.a]
extends = "b"
[profiles.b]
extends = "a"
"#,
        );
        let manager = ConfigManager::new(FixedLocator {
            sources: vec![ConfigSource {
                origin: ConfigOrigin::Explicit,
                trust: ConfigTrust::ExplicitlySelected,
                path: cycle,
            }],
        });
        assert!(manager
            .load(ConfigRequest::default(), ConfigEnvironment::default())
            .expect_err("cycle must fail")
            .to_string()
            .contains("inheritance cycle"));
    }

    #[test]
    fn config_errors_do_not_require_a_terminal_or_pty_object() {
        let path = temp_file(
            "invalid.toml",
            r#"
schema_version = 1
[profiles.interactive.render]
soft_latency_budget_ms = 2000
hard_timeout_ms = 1000
"#,
        );
        let manager = ConfigManager::new(FixedLocator {
            sources: vec![ConfigSource {
                origin: ConfigOrigin::Explicit,
                trust: ConfigTrust::ExplicitlySelected,
                path,
            }],
        });
        let error = manager
            .load(ConfigRequest::default(), ConfigEnvironment::default())
            .expect_err("invalid budgets must fail");
        assert!(error.to_string().contains("soft_latency_budget_ms"));
    }

    #[test]
    fn filesystem_locator_never_auto_loads_project_configuration() {
        let locator = FilesystemConfigLocator;
        let root = std::env::temp_dir().join("ptymark-untrusted-project");
        let request = ConfigRequest {
            working_directory: root.clone(),
            ..ConfigRequest::default()
        };
        let candidates = locator.candidate_paths(&request, &ConfigEnvironment::default());
        assert!(candidates.iter().any(|source| {
            source.origin == ConfigOrigin::Project
                && source.trust == ConfigTrust::UntrustedProject
                && source.path == root.join(".ptymark.toml")
        }));
        let located = locator
            .locate(&request, &ConfigEnvironment::default())
            .expect("locate");
        assert!(located
            .iter()
            .all(|source| source.origin != ConfigOrigin::Project));
    }
}
