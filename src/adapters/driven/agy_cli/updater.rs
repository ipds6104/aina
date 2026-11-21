//! Driven adapter for Canary-Verified Antigravity CLI (`agy`) updates,
//! version observability probing, SHA-512 verification, atomic swap, and rollback.

use crate::core::domain::agy_updater::{
    constant_time_hex_eq, detect_manifest_platform, evaluate_version_status,
    extract_cli_changelog_excerpt, validate_agy_help_contract, validate_canary_json_response,
    AgyCliHealthReport, AgyManifest, AgyUpdateResult, UpdatePolicyDecision,
};
use sha2::{Digest, Sha512};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{info, warn};

pub const DEFAULT_MANIFEST_BASE_URL: &str =
    "https://antigravity-cli-auto-updater-974169037036.us-central1.run.app";
pub const DEFAULT_CHANGELOG_URL: &str = "https://antigravity.google/docs/changelog.md";
const HEALTH_CACHE_TTL_SECS: i64 = 6 * 3600;

static HEALTH_CACHE: OnceLock<Mutex<Option<(i64, AgyCliHealthReport)>>> = OnceLock::new();

fn health_cache() -> &'static Mutex<Option<(i64, AgyCliHealthReport)>> {
    HEALTH_CACHE.get_or_init(|| Mutex::new(None))
}

/// Returns true unless the operator explicitly opted into background self-update.
pub fn is_auto_update_disabled_by_policy() -> bool {
    !std::env::var("AINA_AGY_ALLOW_AUTO_UPDATE")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
}

/// Returns true when the operator enabled automated canary-verified upgrades.
pub fn is_auto_canary_update_enabled() -> bool {
    std::env::var("AINA_AGY_AUTO_CANARY_UPDATE")
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
}

pub struct AgyUpdater;

impl AgyUpdater {
    /// Resolves the canonical filesystem path of the active `agy` binary.
    /// Follows symlinks (e.g., `/usr/local/bin/agy -> /app/data/bin/agy`) so atomic
    /// replacement updates the real target without breaking symlinks.
    pub fn resolve_canonical_binary_path(configured: &Path) -> PathBuf {
        if let Ok(canon) = std::fs::canonicalize(configured) {
            return canon;
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let candidates = [
            PathBuf::from("/app/data/bin/agy"),
            PathBuf::from("/usr/local/bin/agy"),
            PathBuf::from(&home).join(".local/bin/agy"),
            PathBuf::from("/root/.local/bin/agy"),
        ];
        for cand in candidates {
            if cand.exists() {
                return std::fs::canonicalize(&cand).unwrap_or(cand);
            }
        }
        configured.to_path_buf()
    }

    pub fn backup_path_for(canonical_bin: &Path) -> PathBuf {
        let file_name = canonical_bin
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("agy");
        canonical_bin.with_file_name(format!("{}.prev", file_name))
    }

    pub async fn probe_installed_version(binary_path: &Path) -> Option<String> {
        let mut cmd = Command::new(binary_path);
        cmd.arg("--version")
            .env("AGY_CLI_DISABLE_AUTO_UPDATE", "1")
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(6), cmd.output())
            .await
            .ok()?
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    pub fn read_local_updater_status() -> Option<String> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let status_path = PathBuf::from(home)
            .join(".gemini")
            .join("antigravity-cli")
            .join("updater")
            .join("update_status.json");
        let raw = std::fs::read_to_string(status_path).ok()?;
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        val.get("message")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn detect_current_platform() -> Option<&'static str> {
        let is_musl = Path::new("/lib/libc.musl-x86_64.so.1").exists()
            || Path::new("/lib/libc.musl-aarch64.so.1").exists();
        detect_manifest_platform(std::env::consts::OS, std::env::consts::ARCH, is_musl)
    }

    pub async fn fetch_latest_manifest_from(base_url: &str) -> anyhow::Result<AgyManifest> {
        let platform = Self::detect_current_platform().ok_or_else(|| {
            anyhow::anyhow!(
                "Unsupported platform for agy manifest: {}/{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            )
        })?;
        let manifest_url = format!(
            "{}/manifests/{}.json",
            base_url.trim_end_matches('/'),
            platform
        );
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Aina-Agy-Updater/0.2.0")
            .build()?;

        let resp = client.get(&manifest_url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "Manifest endpoint {} returned HTTP {}",
                manifest_url,
                resp.status()
            );
        }
        let manifest = resp.json::<AgyManifest>().await?;
        if manifest.version.trim().is_empty()
            || manifest.url.trim().is_empty()
            || manifest.sha512.trim().is_empty()
        {
            anyhow::bail!("Manifest payload missing required version/url/sha512 fields");
        }
        Ok(manifest)
    }

    pub async fn fetch_latest_manifest() -> anyhow::Result<AgyManifest> {
        let base_url = std::env::var("AINA_AGY_MANIFEST_BASE_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_MANIFEST_BASE_URL.to_string());
        Self::fetch_latest_manifest_from(&base_url).await
    }

    pub async fn fetch_changelog_excerpt() -> Option<String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("Aina-Agy-Updater/0.2.0")
            .build()
            .ok()?;
        let resp = client.get(DEFAULT_CHANGELOG_URL).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let md = resp.text().await.ok()?;
        extract_cli_changelog_excerpt(&md, 16)
    }

    pub async fn invalidate_cache() {
        let mut lock = health_cache().lock().await;
        *lock = None;
    }

    /// Returns cached version health report (6-hour TTL) unless `force_refresh` is true.
    pub async fn get_health_report_cached(
        configured_bin: &Path,
        force_refresh: bool,
    ) -> AgyCliHealthReport {
        let now = chrono::Utc::now().timestamp();
        if !force_refresh {
            let lock = health_cache().lock().await;
            if let Some((cached_at, ref report)) = *lock {
                if now.saturating_sub(cached_at) < HEALTH_CACHE_TTL_SECS {
                    return report.clone();
                }
            }
        }

        let canonical = Self::resolve_canonical_binary_path(configured_bin);
        let installed = Self::probe_installed_version(&canonical).await;
        let (latest, manifest_error) = match Self::fetch_latest_manifest().await {
            Ok(m) => (Some(m.version), None),
            Err(e) => (None, Some(e.to_string())),
        };

        let decision = evaluate_version_status(installed.as_deref(), latest.as_deref());
        let update_available = matches!(decision, UpdatePolicyDecision::UpdateAvailable { .. });
        let backup_path = Self::backup_path_for(&canonical);

        let report = AgyCliHealthReport {
            binary_path: configured_bin.to_string_lossy().to_string(),
            canonical_path: canonical.to_string_lossy().to_string(),
            installed_version: installed,
            latest_version: latest,
            update_available,
            decision,
            auto_update_disabled: is_auto_update_disabled_by_policy(),
            auto_canary_enabled: is_auto_canary_update_enabled(),
            has_rollback_backup: backup_path.is_file(),
            local_updater_status: Self::read_local_updater_status(),
            manifest_error,
            checked_at_epoch: now,
        };

        let mut lock = health_cache().lock().await;
        *lock = Some((now, report.clone()));
        report
    }

    /// Executes a complete SHA-512 + Canary-Verified atomic upgrade of the `agy` binary.
    pub async fn perform_canary_update(
        configured_bin: &Path,
        dry_run: bool,
        force: bool,
        skip_llm_canary: bool,
    ) -> anyhow::Result<AgyUpdateResult> {
        let base_url = std::env::var("AINA_AGY_MANIFEST_BASE_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_MANIFEST_BASE_URL.to_string());
        Self::perform_canary_update_with_base(
            configured_bin,
            &base_url,
            dry_run,
            force,
            skip_llm_canary,
        )
        .await
    }

    pub async fn perform_canary_update_with_base(
        configured_bin: &Path,
        manifest_base_url: &str,
        dry_run: bool,
        force: bool,
        skip_llm_canary: bool,
    ) -> anyhow::Result<AgyUpdateResult> {
        let canonical = Self::resolve_canonical_binary_path(configured_bin);
        let installed = Self::probe_installed_version(&canonical).await;
        let manifest = Self::fetch_latest_manifest_from(manifest_base_url).await?;
        let decision = evaluate_version_status(installed.as_deref(), Some(&manifest.version));

        if !force {
            match decision {
                UpdatePolicyDecision::UpToDate { installed, latest } => {
                    return Ok(AgyUpdateResult {
                        ok: true,
                        status: "up_to_date".to_string(),
                        previous_version: Some(installed),
                        new_version: Some(latest.clone()),
                        sha512_verified: false,
                        canary_help_passed: false,
                        canary_llm_passed: None,
                        backup_path: None,
                        changelog_excerpt: None,
                        message: format!("agy CLI sudah versi terbaru ({}).", latest),
                    });
                }
                UpdatePolicyDecision::InstalledNewerThanManifest { installed, latest } => {
                    return Ok(AgyUpdateResult {
                        ok: true,
                        status: "newer_installed".to_string(),
                        previous_version: Some(installed.clone()),
                        new_version: Some(latest.clone()),
                        sha512_verified: false,
                        canary_help_passed: false,
                        canary_llm_passed: None,
                        backup_path: None,
                        changelog_excerpt: None,
                        message: format!(
                            "Versi terpasang ({}) lebih baru dari manifest ({}); downgrade dicegah.",
                            installed, latest
                        ),
                    });
                }
                _ => {}
            }
        }

        if dry_run {
            let changelog = Self::fetch_changelog_excerpt().await;
            return Ok(AgyUpdateResult {
                ok: true,
                status: "dry_run".to_string(),
                previous_version: installed.clone(),
                new_version: Some(manifest.version.clone()),
                sha512_verified: false,
                canary_help_passed: false,
                canary_llm_passed: None,
                backup_path: Some(Self::backup_path_for(&canonical).to_string_lossy().to_string()),
                changelog_excerpt: changelog,
                message: format!(
                    "[DRY-RUN] Pembaruan agy CLI siap dijalankan: {} -> {} (target: {}).",
                    installed.as_deref().unwrap_or("unknown"),
                    manifest.version,
                    canonical.display()
                ),
            });
        }

        // Isolated staging workspace (cleaned up automatically at end of scope)
        let staging_dir = std::env::temp_dir().join(format!(
            "aina_agy_staging_{}_{}",
            std::process::id(),
            rand::random::<u32>()
        ));
        tokio::fs::create_dir_all(&staging_dir).await?;

        let outcome = Self::run_staged_canary_and_swap(
            &canonical,
            &installed,
            &manifest,
            &staging_dir,
            skip_llm_canary,
        )
        .await;

        let _ = tokio::fs::remove_dir_all(&staging_dir).await;
        if matches!(&outcome, Ok(r) if r.ok && r.status == "updated") {
            Self::invalidate_cache().await;
        }
        outcome
    }

    async fn run_staged_canary_and_swap(
        canonical: &Path,
        installed: &Option<String>,
        manifest: &AgyManifest,
        staging_dir: &Path,
        skip_llm_canary: bool,
    ) -> anyhow::Result<AgyUpdateResult> {
        let is_tar_gz = manifest.url.contains(".tar.gz");
        let download_path = staging_dir.join(if is_tar_gz { "agy.tar.gz" } else { "agy.bin" });

        // 1. Download payload
        info!(
            "Downloading Antigravity CLI {} from {}",
            manifest.version, manifest.url
        );
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .user_agent("Aina-Agy-Updater/0.2.0")
            .build()?;
        let resp = client.get(&manifest.url).send().await?;
        if !resp.status().is_success() {
            anyhow::bail!(
                "Failed to download agy release payload ({}): HTTP {}",
                manifest.url,
                resp.status()
            );
        }
        let bytes = resp.bytes().await?;

        // 2. Verify SHA-512 checksum before extracting or executing
        let digest = Sha512::digest(&bytes);
        let mut computed_hex = String::with_capacity(128);
        for b in digest.iter() {
            use std::fmt::Write;
            let _ = write!(computed_hex, "{:02x}", b);
        }
        if !constant_time_hex_eq(&computed_hex, &manifest.sha512) {
            warn!(
                "SHA-512 mismatch for agy {}! Aborting upgrade cleanly.",
                manifest.version
            );
            return Ok(AgyUpdateResult {
                ok: false,
                status: "checksum_mismatch".to_string(),
                previous_version: installed.clone(),
                new_version: Some(manifest.version.clone()),
                sha512_verified: false,
                canary_help_passed: false,
                canary_llm_passed: None,
                backup_path: None,
                changelog_excerpt: None,
                message: format!(
                    "Upgrade dibatalkan: SHA-512 payload tidak cocok dengan manifest untuk versi {}.",
                    manifest.version
                ),
            });
        }

        tokio::fs::write(&download_path, &bytes).await?;

        // 3. Extract staging binary
        let staging_bin = if is_tar_gz {
            let extract_out = Command::new("tar")
                .arg("-xzf")
                .arg(&download_path)
                .arg("-C")
                .arg(staging_dir)
                .output()
                .await?;
            if !extract_out.status.success() {
                anyhow::bail!(
                    "Failed to extract agy tarball: {}",
                    String::from_utf8_lossy(&extract_out.stderr)
                );
            }
            let cand1 = staging_dir.join("antigravity");
            let cand2 = staging_dir.join("agy");
            if cand1.is_file() {
                cand1
            } else if cand2.is_file() {
                cand2
            } else {
                anyhow::bail!("Extracted tarball did not contain `antigravity` or `agy` binary");
            }
        } else {
            download_path.clone()
        };

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&staging_bin, std::fs::Permissions::from_mode(0o755));
        }

        // 4. Canary Gate 1: Verify `--version` & required CLI flags in `--help`
        let staged_ver = Self::probe_installed_version(&staging_bin).await;
        if staged_ver.is_none() {
            return Ok(AgyUpdateResult {
                ok: false,
                status: "canary_failed".to_string(),
                previous_version: installed.clone(),
                new_version: Some(manifest.version.clone()),
                sha512_verified: true,
                canary_help_passed: false,
                canary_llm_passed: None,
                backup_path: None,
                changelog_excerpt: None,
                message: "Canary gagal: biner hasil unduhan tidak bisa menjalankan `--version`."
                    .to_string(),
            });
        }

        let help_out = tokio::time::timeout(
            Duration::from_secs(10),
            Command::new(&staging_bin)
                .arg("--help")
                .env("AGY_CLI_DISABLE_AUTO_UPDATE", "1")
                .kill_on_drop(true)
                .output(),
        )
        .await??;
        let help_text = format!(
            "{}\n{}",
            String::from_utf8_lossy(&help_out.stdout),
            String::from_utf8_lossy(&help_out.stderr)
        );
        if let Err(missing_flags) = validate_agy_help_contract(&help_text) {
            warn!(
                "Canary Gate 1 FAILED for agy {}: missing required flags {:?}",
                manifest.version, missing_flags
            );
            return Ok(AgyUpdateResult {
                ok: false,
                status: "canary_failed".to_string(),
                previous_version: installed.clone(),
                new_version: Some(manifest.version.clone()),
                sha512_verified: true,
                canary_help_passed: false,
                canary_llm_passed: None,
                backup_path: None,
                changelog_excerpt: None,
                message: format!(
                    "Upgrade dibatalkan (Canary Gate 1 gagal): flag wajib Aina hilang pada agy {}: {}. Biner produksi tetap aman.",
                    manifest.version,
                    missing_flags.join(", ")
                ),
            });
        }

        // 5. Canary Gate 2: Live headless JSON output contract test (if OAuth token present & not skipped)
        let canary_llm_passed = if skip_llm_canary {
            None
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let token_path = PathBuf::from(home)
                .join(".gemini")
                .join("antigravity-cli")
                .join("antigravity-oauth-token");
            if !token_path.is_file() {
                None
            } else {
                let llm_out = tokio::time::timeout(
                    Duration::from_secs(50),
                    Command::new(&staging_bin)
                        .arg("-p")
                        .arg("Reply with exactly one word: OK")
                        .arg("--output-format")
                        .arg("json")
                        .arg("--print-timeout")
                        .arg("40s")
                        .arg("--dangerously-skip-permissions")
                        .env("AGY_CLI_DISABLE_AUTO_UPDATE", "1")
                        .kill_on_drop(true)
                        .output(),
                )
                .await;

                match llm_out {
                    Ok(Ok(out)) if out.status.success() => {
                        let stdout_str = String::from_utf8_lossy(&out.stdout);
                        if let Err(reason) = validate_canary_json_response(&stdout_str) {
                            warn!(
                                "Canary Gate 2 FAILED for agy {}: {}",
                                manifest.version, reason
                            );
                            return Ok(AgyUpdateResult {
                                ok: false,
                                status: "canary_failed".to_string(),
                                previous_version: installed.clone(),
                                new_version: Some(manifest.version.clone()),
                                sha512_verified: true,
                                canary_help_passed: true,
                                canary_llm_passed: Some(false),
                                backup_path: None,
                                changelog_excerpt: None,
                                message: format!(
                                    "Upgrade dibatalkan (Canary Gate 2 gagal): kontrak JSON output rusak pada agy {} ({}). Biner produksi tetap aman.",
                                    manifest.version, reason
                                ),
                            });
                        }
                        Some(true)
                    }
                    Ok(Ok(out)) => {
                        let err_msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
                        return Ok(AgyUpdateResult {
                            ok: false,
                            status: "canary_failed".to_string(),
                            previous_version: installed.clone(),
                            new_version: Some(manifest.version.clone()),
                            sha512_verified: true,
                            canary_help_passed: true,
                            canary_llm_passed: Some(false),
                            backup_path: None,
                            changelog_excerpt: None,
                            message: format!(
                                "Upgrade dibatalkan (Canary Gate 2 exit non-zero): {}. Biner produksi tetap aman.",
                                err_msg
                            ),
                        });
                    }
                    _ => {
                        return Ok(AgyUpdateResult {
                            ok: false,
                            status: "canary_failed".to_string(),
                            previous_version: installed.clone(),
                            new_version: Some(manifest.version.clone()),
                            sha512_verified: true,
                            canary_help_passed: true,
                            canary_llm_passed: Some(false),
                            backup_path: None,
                            changelog_excerpt: None,
                            message: "Upgrade dibatalkan (Canary Gate 2 timeout/error). Biner produksi tetap aman."
                                .to_string(),
                        });
                    }
                }
            }
        };

        // 6. Backup current binary + Zero-Downtime Atomic Same-Filesystem Rename
        let parent_dir = canonical
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid canonical binary path: {:?}", canonical))?;
        tokio::fs::create_dir_all(parent_dir).await?;

        let backup_path = Self::backup_path_for(canonical);
        if canonical.is_file() {
            tokio::fs::copy(canonical, &backup_path).await?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&backup_path, std::fs::Permissions::from_mode(0o755));
            }
        }

        let tmp_target = canonical.with_file_name(format!(
            ".agy.new.{}",
            rand::random::<u32>()
        ));
        tokio::fs::copy(&staging_bin, &tmp_target).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp_target, std::fs::Permissions::from_mode(0o755));
        }
        // Atomic rename on POSIX: in-flight `agy` processes continue executing their open inode cleanly.
        tokio::fs::rename(&tmp_target, canonical).await?;

        let final_ver = Self::probe_installed_version(canonical)
            .await
            .unwrap_or_else(|| manifest.version.clone());
        let changelog = Self::fetch_changelog_excerpt().await;

        info!(
            "Successfully canary-upgraded agy CLI: {} -> {} at {:?} (backup: {:?})",
            installed.as_deref().unwrap_or("unknown"),
            final_ver,
            canonical,
            backup_path
        );

        Ok(AgyUpdateResult {
            ok: true,
            status: "updated".to_string(),
            previous_version: installed.clone(),
            new_version: Some(final_ver.clone()),
            sha512_verified: true,
            canary_help_passed: true,
            canary_llm_passed,
            backup_path: Some(backup_path.to_string_lossy().to_string()),
            changelog_excerpt: changelog,
            message: format!(
                "agy CLI berhasil diperbarui secara atomik: {} -> {} (rollback tersedia di {}).",
                installed.as_deref().unwrap_or("unknown"),
                final_ver,
                backup_path.display()
            ),
        })
    }

    /// Atomically rolls back `agy` to the previous backed-up binary (`agy.prev`).
    pub async fn rollback_binary(configured_bin: &Path) -> anyhow::Result<AgyUpdateResult> {
        let canonical = Self::resolve_canonical_binary_path(configured_bin);
        let backup_path = Self::backup_path_for(&canonical);
        if !backup_path.is_file() {
            return Ok(AgyUpdateResult {
                ok: false,
                status: "no_backup".to_string(),
                previous_version: Self::probe_installed_version(&canonical).await,
                new_version: None,
                sha512_verified: false,
                canary_help_passed: false,
                canary_llm_passed: None,
                backup_path: None,
                changelog_excerpt: None,
                message: format!(
                    "Tidak ditemukan berkas rollback backup di {}.",
                    backup_path.display()
                ),
            });
        }

        let current_ver = Self::probe_installed_version(&canonical).await;
        let backup_ver = Self::probe_installed_version(&backup_path).await;
        if backup_ver.is_none() {
            anyhow::bail!("Backup binary at {:?} failed `--version` check", backup_path);
        }

        let tmp_restore = canonical.with_file_name(format!(".agy.rollback.{}", rand::random::<u32>()));
        tokio::fs::copy(&backup_path, &tmp_restore).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp_restore, std::fs::Permissions::from_mode(0o755));
        }
        tokio::fs::rename(&tmp_restore, &canonical).await?;
        Self::invalidate_cache().await;

        Ok(AgyUpdateResult {
            ok: true,
            status: "rolled_back".to_string(),
            previous_version: current_ver.clone(),
            new_version: backup_ver.clone(),
            sha512_verified: false,
            canary_help_passed: true,
            canary_llm_passed: None,
            backup_path: Some(backup_path.to_string_lossy().to_string()),
            changelog_excerpt: None,
            message: format!(
                "Rollback berhasil secara atomik: {} -> {}.",
                current_ver.as_deref().unwrap_or("unknown"),
                backup_ver.as_deref().unwrap_or("unknown")
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Json, Router};
    use std::os::unix::fs::PermissionsExt;

    fn write_mock_agy(path: &Path, version: &str, include_all_flags: bool) {
        let help_flags = if include_all_flags {
            "--output-format --print-timeout --dangerously-skip-permissions --conversation --model --effort"
        } else {
            "--model --conversation"
        };
        let script = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"{}\"; exit 0; fi\nif [ \"$1\" = \"--help\" ]; then echo \"{}\"; exit 0; fi\nexit 0\n",
            version, help_flags
        );
        std::fs::write(path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn sha512_hex(bytes: &[u8]) -> String {
        let digest = Sha512::digest(bytes);
        let mut out = String::with_capacity(128);
        for b in digest.iter() {
            use std::fmt::Write;
            let _ = write!(out, "{:02x}", b);
        }
        out
    }

    #[tokio::test]
    async fn test_phase21_canary_update_checksum_contract_atomic_swap_and_rollback() {
        let dir = std::env::temp_dir().join(format!("aina_phase21_upd_{}", rand::random::<u32>()));
        std::fs::create_dir_all(&dir).unwrap();

        let prod_bin = dir.join("agy");
        write_mock_agy(&prod_bin, "1.5.10", true);

        let good_payload = b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"1.5.14\"; exit 0; fi\nif [ \"$1\" = \"--help\" ]; then echo \"--output-format --print-timeout --dangerously-skip-permissions --conversation --model --effort\"; exit 0; fi\nexit 0\n".to_vec();
        let good_sha = sha512_hex(&good_payload);

        let broken_payload = b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"1.5.15\"; exit 0; fi\nif [ \"$1\" = \"--help\" ]; then echo \"--only-model\"; exit 0; fi\nexit 0\n".to_vec();
        let broken_sha = sha512_hex(&broken_payload);

        let platform = AgyUpdater::detect_current_platform().unwrap();

        // Start local mock release server serving 3 scenarios: bad checksum, broken contract, valid update
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{}", addr);

        let good_bytes_c = good_payload.clone();
        let broken_bytes_c = broken_payload.clone();
        let good_sha_c = good_sha.clone();
        let broken_sha_c = broken_sha.clone();
        let base_c1 = base.clone();
        let base_c2 = base.clone();
        let base_c3 = base.clone();

        let app = Router::new()
            .route(
                &format!("/bad-sha/manifests/{}.json", platform),
                get(move || {
                    let url = format!("{}/good.bin", base_c1);
                    async move {
                        Json(AgyManifest {
                            version: "1.5.14".into(),
                            url,
                            sha512: "0".repeat(128),
                        })
                    }
                }),
            )
            .route(
                &format!("/broken-flags/manifests/{}.json", platform),
                get(move || {
                    let url = format!("{}/broken.bin", base_c2);
                    let sha = broken_sha_c.clone();
                    async move {
                        Json(AgyManifest {
                            version: "1.5.15".into(),
                            url,
                            sha512: sha,
                        })
                    }
                }),
            )
            .route(
                &format!("/good/manifests/{}.json", platform),
                get(move || {
                    let url = format!("{}/good.bin", base_c3);
                    let sha = good_sha_c.clone();
                    async move {
                        Json(AgyManifest {
                            version: "1.5.14".into(),
                            url,
                            sha512: sha,
                        })
                    }
                }),
            )
            .route("/good.bin", get(move || {
                let b = good_bytes_c.clone();
                async move { b }
            }))
            .route("/broken.bin", get(move || {
                let b = broken_bytes_c.clone();
                async move { b }
            }));

        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        // 1. Checksum mismatch MUST abort without modifying prod binary
        let bad_sha_res = AgyUpdater::perform_canary_update_with_base(
            &prod_bin,
            &format!("{}/bad-sha", base),
            false,
            false,
            true,
        )
        .await
        .unwrap();
        assert!(!bad_sha_res.ok);
        assert_eq!(bad_sha_res.status, "checksum_mismatch");
        assert_eq!(AgyUpdater::probe_installed_version(&prod_bin).await.as_deref(), Some("1.5.10"));

        // 2. Broken CLI flag contract MUST fail Canary Gate 1 without modifying prod binary
        let broken_res = AgyUpdater::perform_canary_update_with_base(
            &prod_bin,
            &format!("{}/broken-flags", base),
            false,
            false,
            true,
        )
        .await
        .unwrap();
        assert!(!broken_res.ok);
        assert_eq!(broken_res.status, "canary_failed");
        assert!(broken_res.sha512_verified);
        assert!(!broken_res.canary_help_passed);
        assert_eq!(AgyUpdater::probe_installed_version(&prod_bin).await.as_deref(), Some("1.5.10"));

        // 3. Valid release passes SHA-512 + Canary Gate 1, backs up 1.5.10, and atomically swaps to 1.5.14
        let ok_res = AgyUpdater::perform_canary_update_with_base(
            &prod_bin,
            &format!("{}/good", base),
            false,
            false,
            true,
        )
        .await
        .unwrap();
        assert!(ok_res.ok, "{}", ok_res.message);
        assert_eq!(ok_res.status, "updated");
        assert_eq!(ok_res.previous_version.as_deref(), Some("1.5.10"));
        assert_eq!(ok_res.new_version.as_deref(), Some("1.5.14"));
        assert_eq!(AgyUpdater::probe_installed_version(&prod_bin).await.as_deref(), Some("1.5.14"));
        assert!(AgyUpdater::backup_path_for(&prod_bin).is_file());

        // 4. Atomic rollback restores 1.5.10 cleanly
        let rb_res = AgyUpdater::rollback_binary(&prod_bin).await.unwrap();
        assert!(rb_res.ok, "{}", rb_res.message);
        assert_eq!(rb_res.status, "rolled_back");
        assert_eq!(AgyUpdater::probe_installed_version(&prod_bin).await.as_deref(), Some("1.5.10"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
