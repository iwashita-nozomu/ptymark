use crate::artifact::{ArtifactExpectation, ArtifactFormat, RenderArtifact};
use crate::cache::{
    ArtifactCache, ArtifactCacheKey, CacheAdmission, CacheStats, InvalidationScope,
};
use crate::engine::{EngineRegistry, EngineSelector, RenderRequest};
use crate::model::SemanticBlock;
use crate::renderer::{RenderContext, RenderError};
use crate::ui::Viewport;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheDisposition {
    Hit,
    MissStored,
    MissNotStored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineAttempt {
    pub engine_id: String,
    pub duration: Duration,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderOutcome {
    pub artifact: RenderArtifact,
    pub cache: CacheDisposition,
    pub attempts: Vec<EngineAttempt>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoordinatorStats {
    pub requests: u64,
    pub cache_hits: u64,
    pub engine_attempts: u64,
    pub successful_renders: u64,
    pub fallback_attempts: u64,
    pub failures: u64,
    pub invalid_artifacts: u64,
    pub engine_time: Duration,
}

pub struct RenderCoordinator {
    registry: EngineRegistry,
    selector: Box<dyn EngineSelector>,
    cache: Box<dyn ArtifactCache>,
    stats: CoordinatorStats,
}

impl RenderCoordinator {
    pub fn new(
        registry: EngineRegistry,
        selector: impl EngineSelector + 'static,
        cache: impl ArtifactCache + 'static,
    ) -> Self {
        Self {
            registry,
            selector: Box::new(selector),
            cache: Box::new(cache),
            stats: CoordinatorStats::default(),
        }
    }

    pub fn with_boxed_cache(
        registry: EngineRegistry,
        selector: impl EngineSelector + 'static,
        cache: Box<dyn ArtifactCache>,
    ) -> Self {
        Self {
            registry,
            selector: Box::new(selector),
            cache,
            stats: CoordinatorStats::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        block: &SemanticBlock,
        context: &RenderContext,
        accepted_formats: &[ArtifactFormat],
        presenter_id: &str,
        capabilities_fingerprint: u64,
    ) -> Result<RenderOutcome, RenderError> {
        self.stats.requests = self.stats.requests.saturating_add(1);
        let candidates = self.selector.candidates(block.kind(), accepted_formats);
        if candidates.is_empty() {
            self.stats.failures = self.stats.failures.saturating_add(1);
            return Err(RenderError::new(format!(
                "no renderer engine candidates are configured for `{}`",
                block.kind()
            )));
        }

        let viewport = Viewport::cells(
            context
                .terminal_width
                .and_then(|width| u16::try_from(width).ok())
                .unwrap_or(80),
            24,
        );
        let mut attempts = Vec::new();

        for (index, engine_id) in candidates.into_iter().enumerate() {
            let Some(descriptor) = self.registry.descriptor(&engine_id).cloned() else {
                attempts.push(EngineAttempt {
                    engine_id,
                    duration: Duration::ZERO,
                    error: Some("engine is not registered".to_owned()),
                });
                continue;
            };
            if !descriptor.supports_kind(block.kind()) {
                attempts.push(EngineAttempt {
                    engine_id,
                    duration: Duration::ZERO,
                    error: Some("engine does not support this block kind".to_owned()),
                });
                continue;
            }
            let Some(format) = descriptor.preferred_format(accepted_formats) else {
                attempts.push(EngineAttempt {
                    engine_id,
                    duration: Duration::ZERO,
                    error: Some("engine has no format accepted by the presenter".to_owned()),
                });
                continue;
            };
            let expectation = ArtifactExpectation {
                engine: &descriptor.identity,
                block_kind: block.kind(),
                format,
                layout_sensitivity: descriptor.layout_sensitivity,
            };

            let key = ArtifactCacheKey::new(
                block,
                &descriptor.identity.id,
                &descriptor.identity.version,
                format,
                viewport,
                descriptor.layout_sensitivity,
                context.theme_fingerprint,
                context.options_fingerprint,
                presenter_id,
                capabilities_fingerprint,
            );

            if let Some(artifact) = self.cache.get(&key) {
                match artifact.validate(&expectation) {
                    Ok(()) => {
                        self.stats.cache_hits = self.stats.cache_hits.saturating_add(1);
                        return Ok(RenderOutcome {
                            artifact,
                            cache: CacheDisposition::Hit,
                            attempts,
                        });
                    }
                    Err(error) => {
                        self.stats.invalid_artifacts =
                            self.stats.invalid_artifacts.saturating_add(1);
                        self.cache.invalidate(&InvalidationScope::EngineVersion {
                            id: descriptor.identity.id.clone(),
                            version: descriptor.identity.version.clone(),
                        });
                        attempts.push(EngineAttempt {
                            engine_id: descriptor.identity.id.clone(),
                            duration: Duration::ZERO,
                            error: Some(format!("cached artifact is invalid: {error}")),
                        });
                    }
                }
            }

            if index > 0 {
                self.stats.fallback_attempts = self.stats.fallback_attempts.saturating_add(1);
            }
            self.stats.engine_attempts = self.stats.engine_attempts.saturating_add(1);
            let request = RenderRequest {
                block,
                context,
                preferred_format: format,
            };
            let started = Instant::now();
            match self.registry.render(&descriptor.identity.id, &request) {
                Ok(artifact) => {
                    let duration = started.elapsed();
                    self.stats.engine_time = self.stats.engine_time.saturating_add(duration);
                    if let Err(error) = artifact.validate(&expectation) {
                        self.stats.invalid_artifacts =
                            self.stats.invalid_artifacts.saturating_add(1);
                        attempts.push(EngineAttempt {
                            engine_id: descriptor.identity.id.clone(),
                            duration,
                            error: Some(format!("engine returned an invalid artifact: {error}")),
                        });
                        continue;
                    }

                    let admission = if artifact.cacheable {
                        self.cache.insert(key, artifact.clone())
                    } else {
                        CacheAdmission::Rejected
                    };
                    self.stats.successful_renders = self.stats.successful_renders.saturating_add(1);
                    attempts.push(EngineAttempt {
                        engine_id: descriptor.identity.id,
                        duration,
                        error: None,
                    });
                    return Ok(RenderOutcome {
                        artifact,
                        cache: match admission {
                            CacheAdmission::Stored => CacheDisposition::MissStored,
                            CacheAdmission::Rejected => CacheDisposition::MissNotStored,
                        },
                        attempts,
                    });
                }
                Err(error) => {
                    let duration = started.elapsed();
                    self.stats.engine_time = self.stats.engine_time.saturating_add(duration);
                    attempts.push(EngineAttempt {
                        engine_id: descriptor.identity.id,
                        duration,
                        error: Some(error.to_string()),
                    });
                }
            }
        }

        self.stats.failures = self.stats.failures.saturating_add(1);
        let diagnostics = attempts
            .iter()
            .map(|attempt| {
                format!(
                    "{}: {}",
                    attempt.engine_id,
                    attempt.error.as_deref().unwrap_or("unknown failure")
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        Err(RenderError::new(format!(
            "all renderer engines failed for `{}`: {diagnostics}",
            block.kind()
        )))
    }

    pub const fn stats(&self) -> CoordinatorStats {
        self.stats
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    pub fn registry(&self) -> &EngineRegistry {
        &self.registry
    }
}
