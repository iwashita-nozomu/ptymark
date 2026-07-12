use crate::artifact::{ArtifactFormat, EngineIdentity, RenderArtifact};
use crate::model::{BlockKind, SemanticBlock};
use crate::renderer::{RenderContext, RenderError};
use crate::ui::LayoutSensitivity;
use std::collections::HashMap;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionModel {
    InProcess,
    OneShotProcess,
    PersistentWorker,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EngineDescriptor {
    pub identity: EngineIdentity,
    pub supported_kinds: Vec<BlockKind>,
    pub formats: Vec<ArtifactFormat>,
    pub layout_sensitivity: LayoutSensitivity,
    pub execution_model: ExecutionModel,
}

impl EngineDescriptor {
    pub fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        supported_kinds: Vec<BlockKind>,
        formats: Vec<ArtifactFormat>,
        layout_sensitivity: LayoutSensitivity,
        execution_model: ExecutionModel,
    ) -> Self {
        Self {
            identity: EngineIdentity::new(id, version),
            supported_kinds,
            formats,
            layout_sensitivity,
            execution_model,
        }
    }

    pub fn supports_kind(&self, kind: BlockKind) -> bool {
        self.supported_kinds.contains(&kind)
    }

    pub fn preferred_format(&self, accepted: &[ArtifactFormat]) -> Option<ArtifactFormat> {
        self.formats
            .iter()
            .copied()
            .find(|format| accepted.contains(format))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RenderRequest<'a> {
    pub block: &'a SemanticBlock,
    pub context: &'a RenderContext,
    pub preferred_format: ArtifactFormat,
}

pub trait RenderEngine: Send {
    fn descriptor(&self) -> &EngineDescriptor;
    fn render(&mut self, request: &RenderRequest<'_>) -> Result<RenderArtifact, RenderError>;
}

#[derive(Default)]
pub struct EngineRegistry {
    engines: HashMap<String, Box<dyn RenderEngine>>,
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, engine: impl RenderEngine + 'static) -> Result<(), RenderError> {
        self.register_boxed(Box::new(engine))
    }

    pub fn register_boxed(&mut self, engine: Box<dyn RenderEngine>) -> Result<(), RenderError> {
        let id = engine.descriptor().identity.id.clone();
        if id.trim().is_empty() {
            return Err(RenderError::new("renderer engine ID cannot be empty"));
        }
        if self.engines.contains_key(&id) {
            return Err(RenderError::new(format!(
                "renderer engine `{id}` is already registered"
            )));
        }
        self.engines.insert(id, engine);
        Ok(())
    }

    pub fn descriptor(&self, id: &str) -> Option<&EngineDescriptor> {
        self.engines.get(id).map(|engine| engine.descriptor())
    }

    pub fn descriptors(&self) -> Vec<EngineDescriptor> {
        let mut descriptors = self
            .engines
            .values()
            .map(|engine| engine.descriptor().clone())
            .collect::<Vec<_>>();
        descriptors.sort_by(|left, right| left.identity.id.cmp(&right.identity.id));
        descriptors
    }

    pub fn contains(&self, id: &str) -> bool {
        self.engines.contains_key(id)
    }

    pub fn render(
        &mut self,
        id: &str,
        request: &RenderRequest<'_>,
    ) -> Result<RenderArtifact, RenderError> {
        let engine = self
            .engines
            .get_mut(id)
            .ok_or_else(|| RenderError::new(format!("renderer engine `{id}` is not registered")))?;
        engine.render(request)
    }

    pub fn len(&self) -> usize {
        self.engines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.engines.is_empty()
    }
}

pub trait EngineSelector: Send {
    fn candidates(&self, kind: BlockKind, accepted_formats: &[ArtifactFormat]) -> Vec<String>;
}

#[derive(Clone, Debug, Default)]
pub struct PolicyEngineSelector {
    rules: HashMap<BlockKind, Vec<String>>,
}

impl PolicyEngineSelector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_candidates<I, S>(&mut self, kind: BlockKind, candidates: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.rules
            .insert(kind, candidates.into_iter().map(Into::into).collect());
    }

    pub fn with_candidates<I, S>(mut self, kind: BlockKind, candidates: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.set_candidates(kind, candidates);
        self
    }
}

impl EngineSelector for PolicyEngineSelector {
    fn candidates(&self, kind: BlockKind, _accepted_formats: &[ArtifactFormat]) -> Vec<String> {
        self.rules.get(&kind).cloned().unwrap_or_default()
    }
}
