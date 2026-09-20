use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

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

pub use super::workspace::{GhDeviceSession, GhPollStatus, SyncResult, WorkspaceEngine, WorkspaceInfo};

pub struct KnowledgeEngine;

impl KnowledgeEngine {
    /// Extracts YAML frontmatter between `---` markers and body
    pub fn parse_frontmatter(content: &str) -> (Option<HashMap<String, serde_yaml::Value>>, String) {
        let trimmed = content.trim_start();
        if !trimmed.starts_with("---") {
            return (None, content.to_string());
        }

        let after_first = &trimmed[3..];
        let rest = after_first.trim_start_matches(|c| c == '\r' || c == '\n');

        if let Some(end_idx) = rest.find("\n---") {
            let yaml_str = &rest[..end_idx];
            let body_start = end_idx + 4;
            let body = rest[body_start..].trim_start_matches(|c| c == '\r' || c == '\n');

            if let Ok(val) = serde_yaml::from_str::<HashMap<String, serde_yaml::Value>>(yaml_str) {
                return (Some(val), body.to_string());
            }
        }

        (None, content.to_string())
    }

    /// Scan universal documentation in `knowledge/universal/*.md` and root `facts.md`, `procedures.md`
    pub fn scan_universal<P: AsRef<Path>>(workspace_dir: P) -> Vec<UniversalDoc> {
        let ws = workspace_dir.as_ref();
        let knowledge_dir = ws.join("knowledge");
        if !knowledge_dir.is_dir() {
            return Vec::new();
        }

        let mut docs = Vec::new();
        let mut candidates = Vec::new();

        // 1. Files in knowledge/universal/
        let uni_dir = knowledge_dir.join("universal");
        if uni_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&uni_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                        candidates.push(path);
                    }
                }
            }
        }

        // 2. Standard top-level knowledge files
        for fname in &["facts.md", "procedures.md", "guidelines.md"] {
            let p = knowledge_dir.join(fname);
            if p.is_file() {
                candidates.push(p);
            }
        }

        for path in candidates {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            if let Ok(content) = std::fs::read_to_string(&path) {
                let (fm, body) = Self::parse_frontmatter(&content);

                let title = fm
                    .as_ref()
                    .and_then(|m| m.get("title").or_else(|| m.get("nama")))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| {
                        body.lines()
                            .find(|l| l.starts_with("# "))
                            .map(|l| l[2..].trim().to_string())
                            .unwrap_or_else(|| filename.clone())
                    });

                let summary = fm
                    .as_ref()
                    .and_then(|m| m.get("summary").or_else(|| m.get("deskripsi")))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| {
                        body.lines()
                            .filter(|l| !l.is_empty() && !l.starts_with('#'))
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(" ")
                    });

                let rel_path = path
                    .strip_prefix(ws)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| format!("knowledge/{}", filename));

                docs.push(UniversalDoc {
                    filename,
                    title,
                    summary,
                    path: rel_path,
                });
            }
        }

        docs.sort_by(|a, b| a.filename.cmp(&b.filename));
        docs
    }

    /// Scan activities across `knowledge/kegiatan/` and `knowledge/activities/`
    pub fn scan_activities<P: AsRef<Path>>(workspace_dir: P) -> Vec<ActivityMetadata> {
        let ws = workspace_dir.as_ref();
        let mut activities = Vec::new();

        let base_dirs = [
            ws.join("knowledge").join("kegiatan"),
            ws.join("knowledge").join("activities"),
            ws.join("kegiatan"),
        ];

        for base in base_dirs {
            if !base.is_dir() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry1 in entries.flatten() {
                    let p1 = entry1.path();
                    if !p1.is_dir() || p1.file_name().unwrap().to_string_lossy().starts_with('.') {
                        continue;
                    }

                    if let Ok(sub_entries) = std::fs::read_dir(&p1) {
                        for entry2 in sub_entries.flatten() {
                            let p2 = entry2.path();
                            if !p2.is_dir() || p2.file_name().unwrap().to_string_lossy().starts_with('.') {
                                continue;
                            }

                            // Try finding README.md or index.md
                            let doc_file = if p2.join("README.md").is_file() {
                                Some(p2.join("README.md"))
                            } else if p2.join("index.md").is_file() {
                                Some(p2.join("index.md"))
                            } else {
                                None
                            };

                            if let Some(doc_path) = doc_file {
                                let name1 = p1.file_name().unwrap().to_string_lossy().to_string();
                                let name2 = p2.file_name().unwrap().to_string_lossy().to_string();

                                // Detect which one is period vs slug
                                let (periode, slug) = if Self::is_period(&name2) {
                                    (name2, name1)
                                } else {
                                    (name1, name2)
                                };

                                if let Ok(content) = std::fs::read_to_string(&doc_path) {
                                    let (fm, _) = Self::parse_frontmatter(&content);
                                    let rel_path = doc_path
                                        .strip_prefix(ws)
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|_| doc_path.to_string_lossy().to_string());

                                    let act = if let Some(m) = fm {
                                        // Check single deadline or deadlines list
                                        let deadline_str = m
                                            .get("deadline")
                                            .and_then(|v| v.as_str())
                                            .map(String::from)
                                            .or_else(|| {
                                                m.get("deadlines").and_then(|v| v.as_sequence()).and_then(|seq| {
                                                    seq.first().and_then(|item| {
                                                        item.get("tanggal")
                                                            .or_else(|| item.get("date"))
                                                            .or_else(|| item.get("deadline"))
                                                            .and_then(|d| d.as_str())
                                                            .map(String::from)
                                                    })
                                                })
                                            });

                                        ActivityMetadata {
                                            nama: m
                                                .get("nama")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(&slug)
                                                .to_string(),
                                            periode: m
                                                .get("periode")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(&periode)
                                                .to_string(),
                                            status: m
                                                .get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("active")
                                                .to_string(),
                                            pic: m
                                                .get("pic")
                                                .or_else(|| m.get("peran"))
                                                .and_then(|v| v.as_str())
                                                .map(String::from),
                                            deadline: deadline_str,
                                            kategori: m
                                                .get("kategori")
                                                .and_then(|v| v.as_str())
                                                .map(String::from),
                                            path: Some(rel_path),
                                        }
                                    } else {
                                        ActivityMetadata {
                                            nama: slug.replace('-', " "),
                                            periode: periode.clone(),
                                            status: "active".to_string(),
                                            pic: None,
                                            deadline: None,
                                            kategori: None,
                                            path: Some(rel_path),
                                        }
                                    };

                                    activities.push(act);
                                }
                            }
                        }
                    }
                }
            }
        }

        activities.sort_by(|a, b| {
            b.periode
                .cmp(&a.periode)
                .then_with(|| a.nama.cmp(&b.nama))
        });
        activities
    }

    fn is_period(s: &str) -> bool {
        (s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()))
            || (s.len() == 7 && s.chars().take(4).all(|c| c.is_ascii_digit()) && &s[4..5] == "-" && s[5..].chars().all(|c| c.is_ascii_digit()))
            || (s.len() == 7 && s.chars().take(4).all(|c| c.is_ascii_digit()) && &s[4..6] == "-Q")
    }

    /// Compiles `knowledge/index.md` deterministically
    pub fn groom_knowledge_base<P: AsRef<Path>>(workspace_dir: P) -> anyhow::Result<String> {
        let ws = workspace_dir.as_ref();
        let ws_name = ws.file_name().unwrap_or_default().to_string_lossy();
        let knowledge_dir = ws.join("knowledge");
        std::fs::create_dir_all(&knowledge_dir)?;

        let universal_docs = Self::scan_universal(ws);
        let activities = Self::scan_activities(ws);
        let archives = crate::core::domain::archive::ArchiveEngine::get_all_stats(ws)
            .unwrap_or_default();

        let mut out = String::new();
        out.push_str(&format!("# Knowledge Base Catalog - {}\n\n", ws_name));
        out.push_str("> Katalog terpadu dokumentasi universal, matriks aktivitas kerja, dan arsip obrolan. Dokumen ini digenerate secara otomatis oleh sistem Aina Engine.\n\n");

        // 1. Universal Docs
        out.push_str("## 1. Dokumentasi Universal & Kebijakan (Universal)\n\n");
        if universal_docs.is_empty() {
            out.push_str("_Belum ada dokumen universal. Tambahkan file markdown di `knowledge/universal/` atau `knowledge/facts.md`._\n\n");
        } else {
            out.push_str("| Dokumen | Judul / Deskripsi | Tautan Berkas |\n");
            out.push_str("|---|---|---|\n");
            for doc in &universal_docs {
                let summary_preview = if doc.summary.chars().count() > 70 {
                    let trunc: String = doc.summary.chars().take(67).collect();
                    format!("{}...", trunc)
                } else {
                    doc.summary.clone()
                };
                out.push_str(&format!(
                    "| **{}** | {} | [`{}`]({}) |\n",
                    doc.title, summary_preview, doc.filename, doc.path
                ));
            }
            out.push('\n');
        }

        // 2. Activities Matrix
        out.push_str("## 2. Matriks Aktivitas & Program Kerja (Activities)\n\n");
        if activities.is_empty() {
            out.push_str("_Belum ada aktivitas tercatat. Buat folder di `knowledge/kegiatan/<nama>/<periode>/README.md`._\n\n");
        } else {
            out.push_str("| Periode | Nama Aktivitas | PIC / Peran | Status | Tenggat Waktu | Berkas |\n");
            out.push_str("|---|---|---|---|---|---|\n");
            for act in &activities {
                let pic_str = act.pic.as_deref().unwrap_or("-");
                let deadline_str = act.deadline.as_deref().unwrap_or("-");
                let path_str = act.path.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| `{}` | **{}** | {} | `{}` | `{}` | [`README.md`]({}) |\n",
                    act.periode, act.nama, pic_str, act.status, deadline_str, path_str
                ));
            }
            out.push('\n');
        }

        // 3. Upcoming Deadlines
        out.push_str("## 3. Ringkasan Tenggat Waktu Mendatang (Deadlines)\n\n");
        let mut with_deadlines: Vec<_> = activities
            .iter()
            .filter(|a| a.deadline.is_some() && a.status != "selesai" && a.status != "completed" && a.status != "archived")
            .collect();

        with_deadlines.sort_by(|a, b| a.deadline.cmp(&b.deadline));

        if with_deadlines.is_empty() {
            out.push_str("_Tidak ada tenggat waktu aktif yang mendesak._\n\n");
        } else {
            for act in with_deadlines {
                let d = act.deadline.as_deref().unwrap_or("TBD");
                let pic = act.pic.as_deref().unwrap_or("Tim");
                out.push_str(&format!(
                    "- **{}**: {} (Periode: `{}`, PIC: {})\n",
                    d, act.nama, act.periode, pic
                ));
            }
            out.push('\n');
        }

        // 4. Archives
        out.push_str("## 4. Arsip Riwayat Obrolan WhatsApp (SQLite FTS5)\n\n");
        if archives.is_empty() {
            out.push_str("_Belum ada arsip obrolan yang diimpor. Gunakan `aina archive import` untuk mengimpor file export `.zip`._\n\n");
        } else {
            out.push_str("| Nama Arsip | Total Pesan | Partisipan | Rentang Tanggal | Ukuran |\n");
            out.push_str("|---|---|---|---|---|\n");
            for arc in archives {
                let earliest = arc.earliest_date.as_deref().unwrap_or("-");
                let latest = arc.latest_date.as_deref().unwrap_or("-");
                let date_range = format!("{} s.d. {}", earliest, latest);
                let size_kb = (arc.file_size_bytes as f64) / 1024.0;
                out.push_str(&format!(
                    "| **{}** | {} | {} | `{}` | {:.1} KB |\n",
                    arc.archive_name, arc.total_messages, arc.total_participants, date_range, size_kb
                ));
            }
            out.push('\n');
        }

        let index_path = knowledge_dir.join("index.md");
        std::fs::write(&index_path, &out)?;
        info!("Groomed knowledge index written to {:?}", index_path);
        Ok(out)
    }

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
            let activities = Self::scan_activities(ws);
            for act in &activities {
                if !Self::is_period(&act.periode) {
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

            if Self::groom_knowledge_base(ws).is_ok() {
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

    /// Inspect an external or internal workspace and return structured metadata
    pub fn get_workspace_info<P: AsRef<Path>>(workspace_dir: P) -> WorkspaceInfo {
        WorkspaceEngine::get_workspace_info(workspace_dir)
    }

    /// Initialize a new workspace directory anywhere on disk with starter templates
    pub fn init_workspace<P: AsRef<Path>>(target_dir: P, title: Option<&str>) -> anyhow::Result<()> {
        WorkspaceEngine::init_workspace(target_dir, title)
    }

    /// Clone an existing remote Git knowledge base repository into the target workspace
    pub fn clone_workspace<P: AsRef<Path>>(git_url: &str, target_dir: P) -> anyhow::Result<()> {
        WorkspaceEngine::clone_workspace(git_url, target_dir)
    }

    /// Synchronize a git-backed workspace: pull updates, groom index, commit changes, and push
    pub fn sync_workspace<P: AsRef<Path>>(
        workspace_dir: P,
        custom_commit_msg: Option<&str>,
    ) -> anyhow::Result<SyncResult> {
        WorkspaceEngine::sync_workspace(workspace_dir, custom_commit_msg)
    }

    /// Link an existing workspace to a remote Git repository
    pub fn link_workspace<P: AsRef<Path>>(workspace_dir: P, git_url: &str) -> anyhow::Result<()> {
        WorkspaceEngine::link_workspace(workspace_dir, git_url)
    }

    /// Check GitHub CLI (gh) authentication and version status
    pub fn check_gh_status() -> anyhow::Result<String> {
        WorkspaceEngine::check_gh_status()
    }

    /// Non-interactive GitHub CLI login via Personal Access Token (PAT)
    pub fn login_github_token(token: &str) -> anyhow::Result<String> {
        WorkspaceEngine::login_github_token(token)
    }

    /// Starts non-blocking GitHub OAuth Device Authorization Flow (RFC 8628)
    pub fn start_gh_device_flow() -> anyhow::Result<GhDeviceSession> {
        WorkspaceEngine::start_gh_device_flow()
    }

    /// Polls GitHub OAuth token endpoint non-blockingly to check if user authorized
    pub fn poll_gh_device_flow() -> anyhow::Result<GhPollStatus> {
        WorkspaceEngine::poll_gh_device_flow()
    }

    /// Create a remote GitHub repository directly using GitHub CLI (gh)
    pub fn create_github_repo<P: AsRef<Path>>(
        workspace_dir: P,
        repo_name: &str,
        private: bool,
    ) -> anyhow::Result<String> {
        WorkspaceEngine::create_github_repo(workspace_dir, repo_name, private)
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
