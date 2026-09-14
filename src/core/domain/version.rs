use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CURRENT_COMMIT: &str = env!("AINA_GIT_COMMIT");
pub const CURRENT_BRANCH: &str = env!("AINA_GIT_BRANCH");
pub const BUILD_TIMESTAMP: &str = env!("AINA_BUILD_TIME");
pub const GITHUB_REPO: &str = "ipds6104/aina";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: String,
    pub commit_hash: String,
    pub git_branch: String,
    pub build_timestamp: String,
    pub repository: String,
}

impl BuildInfo {
    pub fn current() -> Self {
        Self {
            version: CURRENT_VERSION.to_string(),
            commit_hash: CURRENT_COMMIT.to_string(),
            git_branch: CURRENT_BRANCH.to_string(),
            build_timestamp: BUILD_TIMESTAMP.to_string(),
            repository: format!("https://github.com/{}", GITHUB_REPO),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub introduced_in: String,
    pub status: String,
    pub verification_hint: String,
}

impl CapabilityItem {
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        introduced_in: &str,
        status: &str,
        verification_hint: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            introduced_in: introduced_in.to_string(),
            status: status.to_string(),
            verification_hint: verification_hint.to_string(),
        }
    }
}

pub fn get_active_capabilities() -> Vec<CapabilityItem> {
    vec![
        CapabilityItem::new(
            "dual_whatsapp_gateway",
            "Dual-Gateway WhatsApp & Companion Sensor",
            "Dukungan 2 gateway Whatsmeow sekaligus: nomor bot resmi dan akun pribadi Admin/Companion untuk membaca obrolan grup secara ambient.",
            "v0.2.0 (5d4466a)",
            "stable",
            "python3 skills/whatsmeow/scripts/wa_tool.py groups --companion",
        ),
        CapabilityItem::new(
            "multi_bubble_splitting",
            "Multi-Bubble Message Splitting",
            "Memecah balasan menjadi beberapa balon chat terpisah di WhatsApp menggunakan token delimiter <<<SPLIT_CHAT>>>.",
            "v0.2.0 (b514dc1)",
            "stable",
            "Pisahkan respons dengan <<<SPLIT_CHAT>>> untuk draf siap forward",
        ),
        CapabilityItem::new(
            "claim_check_media_streaming",
            "Claim-Check Media Architecture",
            "Streaming unduhan media lampiran berukuran besar (>512KB) tanpa Head-of-Line blocking di gateway HTTP.",
            "v0.2.0 (994d298)",
            "stable",
            "GET /api/v1/media/{id}/download",
        ),
        CapabilityItem::new(
            "sqlite_fts5_archive",
            "Local SQLite FTS5 Temporal Archive",
            "Pencarian full-text instan BM25 dengan filter rentang waktu (--since, --from, --to) pada database SQLite lokal.",
            "v0.1.0",
            "stable",
            "aina archive search <kata_kunci> --since 7d",
        ),
        CapabilityItem::new(
            "knowledge_base_engine",
            "Knowledge Base Catalog & Linter",
            "Penyusunan katalog otomatis knowledge/index.md, auto-heal linting, dan tracking deadline.",
            "v0.1.0",
            "stable",
            "aina kb lint --auto-heal",
        ),
        CapabilityItem::new(
            "gdrive_sheets_integration",
            "Google Drive & Google Sheets Integration",
            "Pembuatan, pembacaan, dan append data ke spreadsheet Google Sheets serta sinkronisasi file Drive.",
            "v0.1.0",
            "stable",
            "python3 skills/gdrive/scripts/gdrive_tool.py status",
        ),
        CapabilityItem::new(
            "version_self_introspection",
            "Self-Version & Upstream Introspection",
            "Pemeriksaan commit build lokal terhadap upstream GitHub secara mandiri untuk mencegah halusinasi kapabilitas.",
            "v0.2.0",
            "stable",
            "aina version --check",
        ),
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteCommitSummary {
    pub sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheckReport {
    pub current: BuildInfo,
    pub upstream_branch: String,
    pub upstream_latest_commit: Option<String>,
    pub is_up_to_date: bool,
    pub behind_by: Option<usize>,
    pub unpulled_commits: Vec<RemoteCommitSummary>,
    pub capabilities: Vec<CapabilityItem>,
    pub check_status: String,
    pub message: String,
}

pub struct VersionEngine;

impl VersionEngine {
    pub fn get_build_info() -> BuildInfo {
        BuildInfo::current()
    }

    pub fn get_capabilities() -> Vec<CapabilityItem> {
        get_active_capabilities()
    }

    /// Queries GitHub REST API to compare the current build commit against upstream main branch.
    pub async fn check_upstream_status() -> VersionCheckReport {
        let current = BuildInfo::current();
        let capabilities = get_active_capabilities();
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("Aina-Version-Engine/0.2.0")
            .build()
        {
            Ok(c) => c,
            Err(_) => reqwest::Client::new(),
        };

        let mut req_headers = reqwest::header::HeaderMap::new();
        req_headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github.v3+json".parse().unwrap(),
        );

        if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
            let token_clean = token.trim();
            if !token_clean.is_empty() {
                if let Ok(val) = format!("Bearer {}", token_clean).parse() {
                    req_headers.insert(reqwest::header::AUTHORIZATION, val);
                }
            }
        }

        // 1. If current commit is known, try GitHub Compare API: /compare/{current_sha}...main
        if current.commit_hash != "unknown" && !current.commit_hash.trim().is_empty() {
            let compare_url = format!(
                "https://api.github.com/repos/{}/compare/{}...main",
                GITHUB_REPO, current.commit_hash
            );

            let res = client
                .get(&compare_url)
                .headers(req_headers.clone())
                .send()
                .await;

            if let Ok(resp) = res {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let status_str = json
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let behind_by = json
                            .get("behind_by")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as usize;

                        let mut unpulled_commits = Vec::new();
                        if let Some(commits_arr) = json.get("commits").and_then(|v| v.as_array()) {
                            for c in commits_arr {
                                let sha = c.get("sha").and_then(|v| v.as_str()).unwrap_or("");
                                let short_sha = if sha.len() >= 7 { &sha[..7] } else { sha };
                                let msg = c
                                    .get("commit")
                                    .and_then(|m| m.get("message"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .lines()
                                    .next()
                                    .unwrap_or("");
                                let author = c
                                    .get("commit")
                                    .and_then(|m| m.get("author"))
                                    .and_then(|a| a.get("name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("dev");
                                let date = c
                                    .get("commit")
                                    .and_then(|m| m.get("author"))
                                    .and_then(|a| a.get("date"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");

                                unpulled_commits.push(RemoteCommitSummary {
                                    sha: short_sha.to_string(),
                                    message: msg.to_string(),
                                    author: author.to_string(),
                                    date: date.to_string(),
                                });
                            }
                        }

                        let is_up_to_date = status_str == "identical" || behind_by == 0;
                        let latest_commit = unpulled_commits
                            .last()
                            .map(|c| c.sha.clone())
                            .unwrap_or_else(|| current.commit_hash.clone());

                        let message = if is_up_to_date {
                            format!(
                                "Container Aina menjalankan commit terbaru ({}) sesuai branch main di GitHub.",
                                current.commit_hash
                            )
                        } else {
                            format!(
                                "Container Aina tertinggal {} commit dari repo GitHub (terbaru: {}). Lakukan redeploy di Coolify untuk memperbarui.",
                                behind_by, latest_commit
                            )
                        };

                        return VersionCheckReport {
                            current,
                            upstream_branch: "main".to_string(),
                            upstream_latest_commit: Some(latest_commit),
                            is_up_to_date,
                            behind_by: Some(behind_by),
                            unpulled_commits,
                            capabilities,
                            check_status: "SUCCESS".to_string(),
                            message,
                        };
                    }
                }
            }
        }

        // 2. Fallback: Query latest commit directly from /commits/main
        let commits_url = format!("https://api.github.com/repos/{}/commits/main", GITHUB_REPO);
        match client.get(&commits_url).headers(req_headers).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let full_sha = json.get("sha").and_then(|v| v.as_str()).unwrap_or("");
                        let short_sha = if full_sha.len() >= 7 {
                            &full_sha[..7]
                        } else {
                            full_sha
                        };

                        let is_identical = current.commit_hash == short_sha
                            || (current.commit_hash.len() >= 7 && full_sha.starts_with(&current.commit_hash));

                        let message = if is_identical {
                            format!(
                                "Container Aina up-to-date dengan commit terbaru ({}) di repo GitHub.",
                                short_sha
                            )
                        } else {
                            format!(
                                "Commit upstream terbaru adalah {} (container saat ini: {}). Terdapat pembaruan di repo GitHub.",
                                short_sha, current.commit_hash
                            )
                        };

                        VersionCheckReport {
                            current,
                            upstream_branch: "main".to_string(),
                            upstream_latest_commit: Some(short_sha.to_string()),
                            is_up_to_date: is_identical,
                            behind_by: None,
                            unpulled_commits: Vec::new(),
                            capabilities,
                            check_status: "SUCCESS".to_string(),
                            message,
                        }
                    } else {
                        VersionCheckReport::offline_report(current, capabilities, "Gagal mem-parsing data commit GitHub API")
                    }
                } else {
                    let code = resp.status();
                    VersionCheckReport::offline_report(
                        current,
                        capabilities,
                        &format!("GitHub API mengembalikan status HTTP {}", code),
                    )
                }
            }
            Err(e) => VersionCheckReport::offline_report(
                current,
                capabilities,
                &format!("Tidak dapat terhubung ke GitHub API (offline/timeout): {}", e),
            ),
        }
    }
}

impl VersionCheckReport {
    fn offline_report(current: BuildInfo, capabilities: Vec<CapabilityItem>, err_detail: &str) -> Self {
        Self {
            current,
            upstream_branch: "main".to_string(),
            upstream_latest_commit: None,
            is_up_to_date: true, // assume current if offline
            behind_by: None,
            unpulled_commits: Vec::new(),
            capabilities,
            check_status: "OFFLINE_FALLBACK".to_string(),
            message: format!(
                "Pemeriksaan remote tidak dapat diselesaikan ({}); menampilkan informasi build lokal.",
                err_detail
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_info_current() {
        let info = BuildInfo::current();
        assert_eq!(info.version, "0.2.0");
        assert!(!info.commit_hash.is_empty());
        assert!(!info.build_timestamp.is_empty());
    }

    #[test]
    fn test_capabilities_list() {
        let caps = get_active_capabilities();
        assert!(!caps.is_empty());
        assert!(caps.iter().any(|c| c.id == "dual_whatsapp_gateway"));
        assert!(caps.iter().any(|c| c.id == "multi_bubble_splitting"));
        assert!(caps.iter().any(|c| c.id == "version_self_introspection"));
    }
}
