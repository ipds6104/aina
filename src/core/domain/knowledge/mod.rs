pub mod groomer;
pub mod linter;
pub mod parser;
pub mod scanner;

pub use groomer::CatalogGroomer;
pub use linter::KnowledgeLinter;
pub use parser::FrontmatterParser;
pub use scanner::DocumentScanner;
pub use super::workspace::{GhDeviceSession, GhPollStatus, SyncResult, WorkspaceEngine, WorkspaceInfo};

use crate::core::ports::KnowledgePort;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMetadata {
    pub nama: String,
    pub periode: String,
    pub status: String,
    #[serde(default)]
    pub pic: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub kategori: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalDoc {
    pub filename: String,
    pub title: String,
    pub summary: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintViolation {
    pub rule: String,
    pub file: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintReport {
    pub workspace: String,
    pub is_clean: bool,
    pub violations_count: usize,
    pub violations: Vec<LintViolation>,
    pub auto_healed: bool,
}

#[derive(Default, Clone)]
pub struct KnowledgeEngine;

#[allow(dead_code)]
impl KnowledgeEngine {
    pub fn new() -> Self {
        Self
    }

    /// Extracts YAML frontmatter between `---` markers and body
    pub fn parse_frontmatter(content: &str) -> (Option<HashMap<String, serde_yaml::Value>>, String) {
        FrontmatterParser::parse(content)
    }

    /// Scan universal documentation in `knowledge/universal/*.md` and root `facts.md`, `procedures.md`
    pub fn scan_universal<P: AsRef<Path>>(workspace_dir: P) -> Vec<UniversalDoc> {
        DocumentScanner::scan_universal(workspace_dir)
    }

    /// Scan activities across `knowledge/kegiatan/` and `knowledge/activities/`
    pub fn scan_activities<P: AsRef<Path>>(workspace_dir: P) -> Vec<ActivityMetadata> {
        DocumentScanner::scan_activities(workspace_dir)
    }

    /// Compiles `knowledge/index.md` deterministically
    pub fn groom_knowledge_base<P: AsRef<Path>>(workspace_dir: P) -> anyhow::Result<String> {
        CatalogGroomer::groom_catalog(workspace_dir)
    }

    /// Deterministic Linter for Knowledge Base neatness
    pub fn lint<P: AsRef<Path>>(workspace_dir: P, auto_heal: bool) -> LintReport {
        KnowledgeLinter::lint(workspace_dir, auto_heal)
    }

    // Delegations to WorkspaceEngine for backwards compatibility
    pub fn get_workspace_info<P: AsRef<Path>>(workspace_dir: P) -> WorkspaceInfo {
        WorkspaceEngine::get_workspace_info(workspace_dir)
    }

    pub fn init_workspace<P: AsRef<Path>>(target_dir: P, title: Option<&str>) -> anyhow::Result<()> {
        WorkspaceEngine::init_workspace(target_dir, title)
    }

    pub fn clone_workspace<P: AsRef<Path>>(git_url: &str, target_dir: P) -> anyhow::Result<()> {
        WorkspaceEngine::clone_workspace(git_url, target_dir)
    }

    pub fn sync_workspace<P: AsRef<Path>>(
        workspace_dir: P,
        custom_commit_msg: Option<&str>,
    ) -> anyhow::Result<SyncResult> {
        WorkspaceEngine::sync_workspace(workspace_dir, custom_commit_msg)
    }

    pub fn link_workspace<P: AsRef<Path>>(workspace_dir: P, git_url: &str) -> anyhow::Result<()> {
        WorkspaceEngine::link_workspace(workspace_dir, git_url)
    }

    pub fn check_gh_status() -> anyhow::Result<String> {
        WorkspaceEngine::check_gh_status()
    }

    pub fn login_github_token(token: &str) -> anyhow::Result<String> {
        WorkspaceEngine::login_github_token(token)
    }

    pub fn start_gh_device_flow() -> anyhow::Result<GhDeviceSession> {
        WorkspaceEngine::start_gh_device_flow()
    }

    pub fn poll_gh_device_flow() -> anyhow::Result<GhPollStatus> {
        WorkspaceEngine::poll_gh_device_flow()
    }

    pub fn create_github_repo<P: AsRef<Path>>(
        workspace_dir: P,
        repo_name: &str,
        private: bool,
    ) -> anyhow::Result<String> {
        WorkspaceEngine::create_github_repo(workspace_dir, repo_name, private)
    }
}

impl KnowledgePort for KnowledgeEngine {
    fn scan_universal(&self, workspace_dir: &Path) -> Vec<UniversalDoc> {
        DocumentScanner::scan_universal(workspace_dir)
    }

    fn scan_activities(&self, workspace_dir: &Path) -> Vec<ActivityMetadata> {
        DocumentScanner::scan_activities(workspace_dir)
    }

    fn groom_catalog(&self, workspace_dir: &Path) -> anyhow::Result<String> {
        CatalogGroomer::groom_catalog(workspace_dir)
    }

    fn lint(&self, workspace_dir: &Path, auto_heal: bool) -> LintReport {
        KnowledgeLinter::lint(workspace_dir, auto_heal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(200);

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\nnama: Rapat Koordinasi\nperiode: 2026-09\nstatus: active\n---\n# Detail Rapat\nIsi pembahasan.";
        let (fm, body) = KnowledgeEngine::parse_frontmatter(content);
        assert!(fm.is_some());
        let m = fm.unwrap();
        assert_eq!(m.get("nama").unwrap().as_str().unwrap(), "Rapat Koordinasi");
        assert!(body.starts_with("# Detail Rapat"));
    }

    #[test]
    fn test_groom_and_lint_auto_heal() {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("aina_test_kb_compat_{}", id));
        let ws_dir = temp_dir.join("workspaces").join("default");
        std::fs::create_dir_all(&ws_dir).unwrap();

        // 1. Initial lint with auto heal
        let report = KnowledgeEngine::lint(&ws_dir, true);
        assert!(report.is_clean);

        // 2. Add an activity
        let act_dir = ws_dir.join("knowledge").join("kegiatan").join("rapat-akreditasi").join("2026-09");
        std::fs::create_dir_all(&act_dir).unwrap();
        let act_file = act_dir.join("README.md");
        std::fs::write(
            &act_file,
            "---\nnama: Persiapan Akreditasi\nperiode: 2026-09\nstatus: aktif\nperan: Pak Budi\ndeadline: 2026-09-30\n---\n# Persiapan Akreditasi",
        )
        .unwrap();

        // 3. Groom
        let catalog = KnowledgeEngine::groom_knowledge_base(&ws_dir).unwrap();
        assert!(catalog.contains("Persiapan Akreditasi"));
        assert!(catalog.contains("2026-09-30"));

        // 4. Lint should be clean
        let report_after = KnowledgeEngine::lint(&ws_dir, false);
        assert!(report_after.is_clean);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_init_workspace_and_get_info() {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("aina_test_ws_init_{}", id));

        // 1. Initialize new external workspace
        KnowledgeEngine::init_workspace(&temp_dir, Some("BPS Prov Kalbar")).unwrap();

        // 2. Verify files created
        assert!(temp_dir.join("GEMINI.md").is_file());
        assert!(temp_dir.join("knowledge").join("facts.md").is_file());
        assert!(temp_dir.join("knowledge").join("procedures.md").is_file());
        assert!(temp_dir.join("knowledge").join("index.md").is_file());

        // 3. Inspect via get_workspace_info
        let info = KnowledgeEngine::get_workspace_info(&temp_dir);
        assert_eq!(info.universal_docs_count, 2);
        assert!(info.has_index);
        assert!(info.is_clean);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_gh_device_session_serde() {
        let session = GhDeviceSession {
            device_code: "dev_12345".to_string(),
            user_code: "WD72-99B1".to_string(),
            verification_uri: "https://github.com/login/device".to_string(),
            expires_in: 900,
            interval: 5,
            created_at: 1700000000,
        };

        let json = serde_json::to_string(&session).unwrap();
        let deserialized: GhDeviceSession = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.device_code, "dev_12345");
        assert_eq!(deserialized.user_code, "WD72-99B1");
        assert_eq!(deserialized.verification_uri, "https://github.com/login/device");
        assert_eq!(deserialized.expires_in, 900);
        assert_eq!(deserialized.interval, 5);
        assert_eq!(deserialized.created_at, 1700000000);
    }
}
