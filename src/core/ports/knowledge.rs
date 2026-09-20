use crate::core::domain::knowledge::{ActivityMetadata, LintReport, UniversalDoc};
use std::path::Path;

/// Interface-first port for Knowledge Base operations (Hexagonal Architecture)
#[allow(dead_code)]
pub trait KnowledgePort: Send + Sync {
    /// Scan universal documentation in `knowledge/universal/*.md` and root docs
    fn scan_universal(&self, workspace_dir: &Path) -> Vec<UniversalDoc>;

    /// Scan activities across `knowledge/kegiatan/` and `knowledge/activities/`
    fn scan_activities(&self, workspace_dir: &Path) -> Vec<ActivityMetadata>;

    /// Compiles `knowledge/index.md` deterministically
    fn groom_catalog(&self, workspace_dir: &Path) -> anyhow::Result<String>;

    /// Deterministic linter for knowledge base neatness
    fn lint(&self, workspace_dir: &Path, auto_heal: bool) -> LintReport;
}
