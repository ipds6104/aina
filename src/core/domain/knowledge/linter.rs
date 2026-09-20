use super::groomer::CatalogGroomer;
use super::scanner::DocumentScanner;
use super::{LintReport, LintViolation};
use std::path::Path;
use tracing::info;

/// Deterministic Knowledge Base Neatness Linter & Auto-Healer
pub struct KnowledgeLinter;

impl KnowledgeLinter {
    /// Deterministic Linter for Knowledge Base neatness
    pub fn lint<P: AsRef<Path>>(workspace_dir: P, auto_heal: bool) -> LintReport {
        let ws = workspace_dir.as_ref();
        let ws_name = ws.file_name().unwrap_or_default().to_string_lossy().to_string();
        let knowledge_dir = ws.join("knowledge");
        let mut violations = Vec::new();

        if !knowledge_dir.is_dir() {
            violations.push(LintViolation {
                rule: "KB001_DIR_EXISTS".to_string(),
                file: "knowledge".to_string(),
                message: "Direktori `knowledge/` belum dibuat.".to_string(),
                severity: "ERROR".to_string(),
            });
        } else {
            // Check allowed root files in knowledge/: index.md, facts.md, procedures.md, guidelines.md
            let allowed_root = ["index.md", "facts.md", "procedures.md", "guidelines.md"];
            if let Ok(entries) = std::fs::read_dir(&knowledge_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = path.file_name().unwrap().to_string_lossy();
                        if !allowed_root.contains(&fname.as_ref()) {
                            violations.push(LintViolation {
                                rule: "KB003_ROOT_JUNK".to_string(),
                                file: format!("knowledge/{}", fname),
                                message: format!(
                                    "Berkas `{}` berada di root knowledge. Pindahkan ke subdirektori terkait.",
                                    fname
                                ),
                                severity: "WARN".to_string(),
                            });
                        }
                    }
                }
            }

            // Check activities
            let activities = DocumentScanner::scan_activities(ws);
            for act in &activities {
                if !DocumentScanner::is_period(&act.periode) {
                    violations.push(LintViolation {
                        rule: "KB004_PERIOD_FORMAT".to_string(),
                        file: act.path.clone().unwrap_or_default(),
                        message: format!(
                            "Format periode `{}` tidak baku. Gunakan YYYY-MM, YYYY-QX, atau YYYY.",
                            act.periode
                        ),
                        severity: "WARN".to_string(),
                    });
                }

                if let Some(ref dl) = act.deadline {
                    if dl.len() != 10 || &dl[4..5] != "-" || &dl[7..8] != "-" {
                        violations.push(LintViolation {
                            rule: "KB009_FM_DEADLINE".to_string(),
                            file: act.path.clone().unwrap_or_default(),
                            message: format!("Format deadline `{}` tidak valid. Gunakan ISO format YYYY-MM-DD.", dl),
                            severity: "WARN".to_string(),
                        });
                    }
                }
            }

            // Check index.md
            let index_md = knowledge_dir.join("index.md");
            if !index_md.is_file() {
                violations.push(LintViolation {
                    rule: "KB011_ROOT_INDEX_MISSING".to_string(),
                    file: "knowledge/index.md".to_string(),
                    message: "Katalog `knowledge/index.md` belum dibuat. Perlu digroom.".to_string(),
                    severity: "ERROR".to_string(),
                });
            }
        }

        let mut auto_healed = false;
        if auto_heal && !violations.is_empty() {
            info!("Auto-healing knowledge base for workspace: {}", ws_name);
            let _ = std::fs::create_dir_all(knowledge_dir.join("universal"));
            let _ = std::fs::create_dir_all(knowledge_dir.join("kegiatan"));
            let _ = std::fs::create_dir_all(knowledge_dir.join("archives"));

            if CatalogGroomer::groom_catalog(ws).is_ok() {
                let mut re_report = Self::lint(workspace_dir, false);
                re_report.auto_healed = true;
                return re_report;
            }
            auto_healed = false;
        }

        let is_clean = violations.is_empty();
        let violations_count = violations.len();

        LintReport {
            workspace: ws_name,
            is_clean,
            violations_count,
            violations,
            auto_healed,
        }
    }
}
