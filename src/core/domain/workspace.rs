use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub path: String,
    pub absolute_path: String,
    pub is_git_repo: bool,
    pub git_remote: Option<String>,
    pub universal_docs_count: usize,
    pub activities_count: usize,
    pub archives_count: usize,
    pub has_index: bool,
    pub is_clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub is_git: bool,
    pub pulled: bool,
    pub pull_summary: String,
    pub committed: bool,
    pub commit_message: Option<String>,
    pub pushed: bool,
    pub push_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhDeviceSession {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GhPollStatus {
    Success { user: String },
    Pending { user_code: String, verification_uri: String },
    Expired,
    Error(String),
}

pub struct WorkspaceEngine;

impl WorkspaceEngine {
    const GITHUB_CLI_CLIENT_ID: &'static str = "178c6fc778ccc68e1d6a";

    fn get_device_session_path() -> PathBuf {
        std::env::temp_dir().join("aina_gh_device_session.json")
    }

    /// Inspect an external or internal workspace and return structured metadata
    pub fn get_workspace_info<P: AsRef<Path>>(workspace_dir: P) -> WorkspaceInfo {
        let ws = workspace_dir.as_ref();
        let abs_path = std::fs::canonicalize(ws)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ws.to_string_lossy().to_string());

        let git_dir = ws.join(".git");
        let is_git_repo = git_dir.is_dir() || git_dir.is_file();

        let mut git_remote = None;
        if is_git_repo {
            let config_file = if git_dir.is_dir() {
                git_dir.join("config")
            } else {
                ws.join(".git")
            };
            if let Ok(content) = std::fs::read_to_string(&config_file) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("url = ") {
                        git_remote = Some(trimmed[6..].trim().to_string());
                        break;
                    }
                }
            }
        }

        let uni = crate::core::domain::knowledge::KnowledgeEngine::scan_universal(ws);
        let act = crate::core::domain::knowledge::KnowledgeEngine::scan_activities(ws);
        let arc = crate::core::domain::archive::ArchiveEngine::discover_archives(ws);
        let has_index = ws.join("knowledge").join("index.md").is_file();
        let lint_report = crate::core::domain::knowledge::KnowledgeEngine::lint(ws, false);

        WorkspaceInfo {
            path: ws.to_string_lossy().to_string(),
            absolute_path: abs_path,
            is_git_repo,
            git_remote,
            universal_docs_count: uni.len(),
            activities_count: act.len(),
            archives_count: arc.len(),
            has_index,
            is_clean: lint_report.is_clean,
        }
    }

    /// Initialize a new workspace directory anywhere on disk with starter templates
    pub fn init_workspace<P: AsRef<Path>>(target_dir: P, title: Option<&str>) -> anyhow::Result<()> {
        let dir = target_dir.as_ref();
        let name = dir.file_name().unwrap_or_default().to_string_lossy().to_string();
        let ws_title = title.unwrap_or(&name);

        let knowledge_dir = dir.join("knowledge");
        let universal_dir = knowledge_dir.join("universal");
        let kegiatan_dir = knowledge_dir.join("kegiatan");
        let archives_dir = knowledge_dir.join("archives");
        let manifests_dir = knowledge_dir.join("manifests");
        let data_dir = dir.join("data").join("chats");
        let scripts_dir = dir.join("scripts");

        std::fs::create_dir_all(&universal_dir)?;
        std::fs::create_dir_all(&kegiatan_dir)?;
        std::fs::create_dir_all(&archives_dir)?;
        std::fs::create_dir_all(&manifests_dir)?;
        std::fs::create_dir_all(&data_dir)?;
        std::fs::create_dir_all(&scripts_dir)?;

        // Auto-link global shared_data lake if not already present
        let shared_data_link = dir.join("shared_data");
        if !shared_data_link.exists() {
            #[cfg(unix)]
            {
                let _ = std::os::unix::fs::symlink("../../shared_data", &shared_data_link);
            }
        }

        let gemini_file = dir.join("GEMINI.md");
        if !gemini_file.exists() {
            let gemini_content = format!(
                "# Workspace Context: {}\n\n> Ruang kerja khusus domain {}\n\n## Aturan & Pedoman\n1. Semua dokumentasi operasional disimpan dalam format Markdown di folder `knowledge/`.\n2. Riwayat obrolan dan data tabular disimpan di folder `data/`.\n",
                ws_title, ws_title
            );
            std::fs::write(&gemini_file, gemini_content)?;
        }

        let facts_file = knowledge_dir.join("facts.md");
        if !facts_file.exists() {
            let facts_content = format!(
                "# 💡 Fakta & Parameter Kunci: {}\n\n*Dokumentasikan fakta penting, parameter operasional, dan keputusan rapat di sini.*\n",
                ws_title
            );
            std::fs::write(&facts_file, facts_content)?;
        }

        let proc_file = knowledge_dir.join("procedures.md");
        if !proc_file.exists() {
            let proc_content = format!(
                "# 📋 SOP & Prosedur: {}\n\n*Dokumentasikan Standar Operasional Prosedur (SOP) dan alur kerja berkala di sini.*\n\n1. **Persiapan**: Periksa ketersediaan dokumen acuan.\n2. **Eksekusi**: Lakukan tindak lanjut sesuai tupoksi.\n",
                ws_title
            );
            std::fs::write(&proc_file, proc_content)?;
        }

        // Groom initial index
        crate::core::domain::knowledge::KnowledgeEngine::groom_knowledge_base(dir)?;
        Ok(())
    }

    /// Clone an existing remote Git knowledge base repository into the target workspace
    pub fn clone_workspace<P: AsRef<Path>>(git_url: &str, target_dir: P) -> anyhow::Result<()> {
        let dir = target_dir.as_ref();
        if dir.exists() && dir.read_dir()?.next().is_some() {
            anyhow::bail!("Direktori target {:?} sudah ada dan tidak kosong.", dir);
        }

        info!("Cloning knowledge repository from {} to {:?}", git_url, dir);
        let status = std::process::Command::new("git")
            .arg("clone")
            .arg(git_url)
            .arg(dir.as_os_str())
            .status()?;

        if !status.success() {
            anyhow::bail!("Gagal melakukan git clone dari {}", git_url);
        }

        // Auto-heal / Groom to ensure index.md exists
        crate::core::domain::knowledge::KnowledgeEngine::lint(dir, true);
        Ok(())
    }

    /// Synchronize a git-backed workspace: pull updates, groom index, commit changes, and push
    pub fn sync_workspace<P: AsRef<Path>>(
        workspace_dir: P,
        custom_commit_msg: Option<&str>,
    ) -> anyhow::Result<SyncResult> {
        let ws = workspace_dir.as_ref();
        let git_dir = ws.join(".git");
        if !git_dir.exists() {
            anyhow::bail!(
                "Workspace {:?} bukan repositori Git. Gunakan `aina workspace link <remote-url>` terlebih dahulu.",
                ws
            );
        }

        // 1. Pull latest changes from remote origin
        let pull_output = std::process::Command::new("git")
            .current_dir(ws)
            .args(["pull", "--rebase", "--autostash"])
            .output();

        let (pulled, pull_summary) = match pull_output {
            Ok(out) => {
                let msg = String::from_utf8_lossy(if out.status.success() { &out.stdout } else { &out.stderr }).to_string();
                (out.status.success(), msg.trim().to_string())
            }
            Err(e) => (false, format!("Gagal mengeksekusi git pull: {}", e)),
        };

        // 2. Groom knowledge index to integrate any pulled changes
        let _ = crate::core::domain::knowledge::KnowledgeEngine::groom_knowledge_base(ws);

        // 3. Stage safe whitelisted documentation files only (Anti-Leak)
        let _ = std::process::Command::new("git")
            .current_dir(ws)
            .args(["add", "knowledge/", "GEMINI.md", "README.md", ".gitignore"])
            .output();

        // 4. Check if there are staged changes
        let status_output = std::process::Command::new("git")
            .current_dir(ws)
            .args(["status", "--porcelain"])
            .output();

        let mut committed = false;
        let mut final_commit_msg = None;

        if let Ok(st) = status_output {
            let changes = String::from_utf8_lossy(&st.stdout);
            if !changes.trim().is_empty() {
                let msg = custom_commit_msg
                    .map(String::from)
                    .unwrap_or_else(|| "docs(kb): auto-sync knowledge updates via Aina [skip ci]".to_string());

                let commit_res = std::process::Command::new("git")
                    .current_dir(ws)
                    .args(["commit", "-m", &msg])
                    .output();

                if let Ok(c_out) = commit_res {
                    if c_out.status.success() {
                        committed = true;
                        final_commit_msg = Some(msg);
                    }
                }
            }
        }

        // 5. Push to remote origin
        let push_output = std::process::Command::new("git")
            .current_dir(ws)
            .args(["push", "origin", "HEAD"])
            .output();

        let (pushed, push_summary) = match push_output {
            Ok(out) => {
                let msg = String::from_utf8_lossy(if out.status.success() { &out.stdout } else { &out.stderr }).to_string();
                (out.status.success(), msg.trim().to_string())
            }
            Err(e) => (false, format!("Gagal mengeksekusi git push: {}", e)),
        };

        Ok(SyncResult {
            is_git: true,
            pulled,
            pull_summary,
            committed,
            commit_message: final_commit_msg,
            pushed,
            push_summary,
        })
    }

    /// Link an existing workspace to a remote Git repository
    pub fn link_workspace<P: AsRef<Path>>(workspace_dir: P, git_url: &str) -> anyhow::Result<()> {
        let ws = workspace_dir.as_ref();
        let git_dir = ws.join(".git");

        // 1. Initialize git if not present
        if !git_dir.exists() {
            let init_status = std::process::Command::new("git")
                .current_dir(ws)
                .args(["init", "-b", "main"])
                .status()?;
            if !init_status.success() {
                anyhow::bail!("Gagal menjalankan git init di {:?}", ws);
            }
        }

        // 2. Ensure default .gitignore exists
        let gitignore_file = ws.join(".gitignore");
        if !gitignore_file.exists() {
            let gitignore_content = "\
# Ignored workspace runtime data
/data/*
!/data/.gitkeep
/output/*
!/output/.gitkeep
*.db
*.db-shm
*.db-wal
.env
__pycache__/
*.pyc
";
            std::fs::write(&gitignore_file, gitignore_content)?;
        }

        // 3. Set or update remote origin
        let remote_check = std::process::Command::new("git")
            .current_dir(ws)
            .args(["remote"])
            .output()?;

        let remotes = String::from_utf8_lossy(&remote_check.stdout);
        if remotes.contains("origin") {
            std::process::Command::new("git")
                .current_dir(ws)
                .args(["remote", "set-url", "origin", git_url])
                .status()?;
        } else {
            std::process::Command::new("git")
                .current_dir(ws)
                .args(["remote", "add", "origin", git_url])
                .status()?;
        }

        // 4. Initial commit of documentation
        let _ = std::process::Command::new("git")
            .current_dir(ws)
            .args(["add", "knowledge/", "GEMINI.md", "README.md", ".gitignore"])
            .output();

        let _ = std::process::Command::new("git")
            .current_dir(ws)
            .args(["commit", "-m", "chore: initialize workspace knowledge vault"])
            .output();

        info!("Linked workspace {:?} to remote origin {}", ws, git_url);
        Ok(())
    }

    /// Check GitHub CLI (gh) authentication and version status
    pub fn check_gh_status() -> anyhow::Result<String> {
        let which_gh = std::process::Command::new("which").arg("gh").output();
        if which_gh.is_err() || !which_gh.unwrap().status.success() {
            anyhow::bail!("GitHub CLI (`gh`) belum terpasang di sistem. Pasang via `apt install gh` atau https://cli.github.com");
        }

        let auth_output = std::process::Command::new("gh")
            .args(["auth", "status"])
            .output()?;

        let text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&auth_output.stdout),
            String::from_utf8_lossy(&auth_output.stderr)
        );

        Ok(text.trim().to_string())
    }

    /// Non-interactive GitHub CLI login via Personal Access Token (PAT)
    pub fn login_github_token(token: &str) -> anyhow::Result<String> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            anyhow::bail!("GitHub Personal Access Token tidak boleh kosong.");
        }
        let mut child = std::process::Command::new("gh")
            .args(["auth", "login", "--with-token"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin.write_all(trimmed.as_bytes())?;
            stdin.flush()?;
        }

        let output = child.wait_with_output()?;
        let res = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if !output.status.success() {
            anyhow::bail!("Gagal login GitHub CLI: {}", res.trim());
        }
        Ok("Berhasil login GitHub CLI menggunakan Personal Access Token!".to_string())
    }

    /// Starts non-blocking GitHub OAuth Device Authorization Flow (RFC 8628)
    pub fn start_gh_device_flow() -> anyhow::Result<GhDeviceSession> {
        let output = std::process::Command::new("curl")
            .args([
                "-s",
                "-X", "POST",
                "https://github.com/login/device/code",
                "-H", "Accept: application/json",
                "-d", &format!("client_id={}&scope=repo,read:org,gist", Self::GITHUB_CLI_CLIENT_ID),
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Gagal menghubungi GitHub Device API via curl.");
        }

        let body_str = String::from_utf8_lossy(&output.stdout);
        let resp: serde_json::Value = serde_json::from_str(&body_str)?;

        if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
            anyhow::bail!("GitHub API error: {}", err);
        }

        let device_code = resp.get("device_code").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let user_code = resp.get("user_code").and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let verification_uri = resp.get("verification_uri").and_then(|v| v.as_str()).unwrap_or("https://github.com/login/device").to_string();
        let expires_in = resp.get("expires_in").and_then(|v| v.as_u64()).unwrap_or(899);
        let interval = resp.get("interval").and_then(|v| v.as_u64()).unwrap_or(5);

        if device_code.is_empty() || user_code.is_empty() {
            anyhow::bail!("Respons GitHub tidak memuat device_code atau user_code: {}", body_str);
        }

        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let session = GhDeviceSession {
            device_code,
            user_code,
            verification_uri,
            expires_in,
            interval,
            created_at: now_sec,
        };

        let session_file = Self::get_device_session_path();
        std::fs::write(&session_file, serde_json::to_string_pretty(&session)?)?;

        Ok(session)
    }

    /// Polls GitHub OAuth token endpoint non-blockingly to check if user authorized
    pub fn poll_gh_device_flow() -> anyhow::Result<GhPollStatus> {
        let session_file = Self::get_device_session_path();
        if !session_file.exists() {
            anyhow::bail!("Tidak ada sesi otorisasi device yang aktif. Jalankan `aina gh-device` untuk memulai.");
        }

        let content = std::fs::read_to_string(&session_file)?;
        let session: GhDeviceSession = serde_json::from_str(&content)?;

        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if now_sec > session.created_at + session.expires_in {
            let _ = std::fs::remove_file(&session_file);
            return Ok(GhPollStatus::Expired);
        }

        let output = std::process::Command::new("curl")
            .args([
                "-s",
                "-X", "POST",
                "https://github.com/login/oauth/access_token",
                "-H", "Accept: application/json",
                "-d", &format!(
                    "client_id={}&device_code={}&grant_type=urn:ietf:params:oauth:grant-type:device_code",
                    Self::GITHUB_CLI_CLIENT_ID,
                    session.device_code
                ),
            ])
            .output()?;

        if !output.status.success() {
            anyhow::bail!("Gagal menghubungi GitHub OAuth Token API via curl.");
        }

        let body_str = String::from_utf8_lossy(&output.stdout);
        let resp: serde_json::Value = serde_json::from_str(&body_str)?;

        if let Some(err) = resp.get("error").and_then(|v| v.as_str()) {
            match err {
                "authorization_pending" => {
                    return Ok(GhPollStatus::Pending {
                        user_code: session.user_code,
                        verification_uri: session.verification_uri,
                    });
                }
                "slow_down" => {
                    return Ok(GhPollStatus::Pending {
                        user_code: session.user_code,
                        verification_uri: session.verification_uri,
                    });
                }
                "expired_token" => {
                    let _ = std::fs::remove_file(&session_file);
                    return Ok(GhPollStatus::Expired);
                }
                other => {
                    return Ok(GhPollStatus::Error(format!("Error dari GitHub: {}", other)));
                }
            }
        }

        if let Some(access_token) = resp.get("access_token").and_then(|v| v.as_str()) {
            // Log in via token
            Self::login_github_token(access_token)?;
            let _ = std::fs::remove_file(&session_file);

            // Check who logged in
            let status = Self::check_gh_status().unwrap_or_default();
            let user = status
                .lines()
                .find(|l| l.contains("account "))
                .and_then(|l| l.split("account ").nth(1))
                .and_then(|l| l.split_whitespace().next())
                .unwrap_or("user")
                .to_string();

            return Ok(GhPollStatus::Success { user });
        }

        Ok(GhPollStatus::Error(format!("Respons tidak dikenal dari GitHub: {}", body_str)))
    }

    /// Create a remote GitHub repository directly using GitHub CLI (gh)
    pub fn create_github_repo<P: AsRef<Path>>(
        workspace_dir: P,
        repo_name: &str,
        private: bool,
    ) -> anyhow::Result<String> {
        let ws = workspace_dir.as_ref();
        let git_dir = ws.join(".git");
        if !git_dir.exists() {
            let _ = std::process::Command::new("git")
                .current_dir(ws)
                .args(["init", "-b", "main"])
                .status();
        }

        // Ensure safe gitignore and initial commit
        let gitignore_file = ws.join(".gitignore");
        if !gitignore_file.exists() {
            let _ = std::fs::write(
                &gitignore_file,
                "/data/*\n!/data/.gitkeep\n/output/*\n!/output/.gitkeep\n*.db\n*.db-shm\n*.db-wal\n.env\n",
            );
        }
        let _ = std::process::Command::new("git")
            .current_dir(ws)
            .args(["add", "knowledge/", "GEMINI.md", "README.md", ".gitignore"])
            .output();
        let _ = std::process::Command::new("git")
            .current_dir(ws)
            .args(["commit", "-m", "chore: initial knowledge base vault"])
            .output();

        let visibility_flag = if private { "--private" } else { "--public" };
        info!("Creating GitHub repository {} via gh CLI...", repo_name);

        let output = std::process::Command::new("gh")
            .current_dir(ws)
            .args([
                "repo",
                "create",
                repo_name,
                visibility_flag,
                "--source",
                ".",
                "--remote",
                "origin",
                "--push",
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Gagal membuat repositori GitHub via gh: {}", err.trim());
        }

        let success_msg = String::from_utf8_lossy(&output.stdout);
        Ok(success_msg.trim().to_string())
    }
}
