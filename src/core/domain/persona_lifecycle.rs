//! Persona, Character, and Organization Configuration Lifecycle & Snapshot Manager.
//!
//! Provides deterministic hot-reloading support, safety snapshots, rolling version history,
//! atomic file modifications, and two-way rollback / factory default restore.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
pub const DEFAULT_TARGET: &str = "persona.md";
pub const SUPPORTED_TARGETS: &[&str] = &[
    "persona.md",
    "character.md",
    "activities.md",
    "organization.md",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersonaSnapshot {
    pub id: String,
    pub target_file: String,
    pub backup_filename: String,
    pub reason: String,
    pub created_at_epoch: u64,
    pub created_at_formatted: String,
    pub byte_size: u64,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackOutcome {
    pub target_file: String,
    pub restored_from_snapshot_id: String,
    pub safety_snapshot_id: String,
    pub restored_byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreDefaultOutcome {
    pub target_file: String,
    pub safety_snapshot_id: String,
    pub template_path: String,
    pub restored_byte_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaFileStatus {
    pub target_file: String,
    pub exists: bool,
    pub path: String,
    pub byte_size: u64,
    pub last_modified: Option<String>,
    pub default_template_exists: bool,
    pub snapshot_count: usize,
    pub latest_snapshot_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PersonaLifecycleManager {
    pub config_dir: PathBuf,
}

impl PersonaLifecycleManager {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// Automatically detects the active configuration directory across Docker, Linux, and local workspaces.
    pub fn from_env() -> Self {
        // 1. Check explicit AGENT_PERSONA_FILE environment variable
        if let Ok(persona_path) = std::env::var("AGENT_PERSONA_FILE") {
            let p = PathBuf::from(persona_path);
            if let Some(parent) = p.parent() {
                if parent.exists() {
                    return Self::new(parent.to_path_buf());
                }
            }
        }

        // 2. Check persistent container storage (/app/data/config)
        let docker_persistent_cfg = PathBuf::from("/app/data/config");
        if docker_persistent_cfg.exists() {
            return Self::new(docker_persistent_cfg);
        }

        // 3. Check /app/config in Docker image
        let app_cfg = PathBuf::from("/app/config");
        if app_cfg.exists() {
            return Self::new(app_cfg);
        }

        // 4. Check local config/ directory
        let local_cfg = PathBuf::from("config");
        if local_cfg.exists() {
            return Self::new(local_cfg);
        }

        // Fallback to current working directory
        Self::new(PathBuf::from("."))
    }

    /// Normalizes alias/short names to canonical file names
    pub fn normalize_target(target: &str) -> &'static str {
        let lower = target.trim().to_lowercase();
        match lower.as_str() {
            "persona" | "persona.md" | "p" => "persona.md",
            "character" | "character.md" | "char" | "c" => "character.md",
            "activities" | "activities.md" | "activity" | "act" | "a" => "activities.md",
            "organization" | "organization.md" | "org" | "o" => "organization.md",
            _ => "persona.md",
        }
    }

    fn history_dir(&self) -> PathBuf {
        self.config_dir.join("history")
    }

    fn manifest_file(&self) -> PathBuf {
        self.history_dir().join("manifest.jsonl")
    }

    /// Generates an immutable snapshot of the target configuration file.
    pub fn create_snapshot(&self, target_name: &str, reason: &str) -> anyhow::Result<PersonaSnapshot> {
        let canonical_target = Self::normalize_target(target_name);
        let target_path = self.config_dir.join(canonical_target);

        if !target_path.exists() {
            anyhow::bail!(
                "Berkas konfigurasi tidak ditemukan di {}. Tidak dapat membuat snapshot.",
                target_path.display()
            );
        }

        let history_dir = self.history_dir();
        fs::create_dir_all(&history_dir)?;

        let now = SystemTime::now();
        let epoch = now.duration_since(UNIX_EPOCH)?.as_secs();
        let stem = canonical_target.strip_suffix(".md").unwrap_or(canonical_target);
        
        let rand_suffix: u32 = rand::random::<u32>() % 9000 + 1000;
        let snapshot_id = format!("{}_{}", epoch, rand_suffix);
        let backup_filename = format!("{}_{}.md", stem, snapshot_id);
        let backup_path = history_dir.join(&backup_filename);

        // Copy active content to timestamped backup
        fs::copy(&target_path, &backup_path)?;

        // Also update .previous.md for instant 1-step undo
        let previous_file = self.config_dir.join(format!("{}.previous.md", stem));
        let _ = fs::copy(&target_path, &previous_file);

        let content = fs::read_to_string(&target_path)?;
        let byte_size = target_path.metadata()?.len();
        let line_count = content.lines().count();
        let formatted_date = format_epoch_local(epoch);

        let snapshot = PersonaSnapshot {
            id: snapshot_id,
            target_file: canonical_target.to_string(),
            backup_filename,
            reason: reason.trim().to_string(),
            created_at_epoch: epoch,
            created_at_formatted: formatted_date,
            byte_size,
            line_count,
        };

        // Append to manifest.jsonl
        let record = serde_json::to_string(&snapshot)?;
        let manifest_path = self.manifest_file();
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(manifest_path)?;
        writeln!(file, "{}", record)?;

        Ok(snapshot)
    }

    /// Lists all snapshots for a given target, sorted newest first.
    pub fn list_snapshots(&self, target_name: &str) -> anyhow::Result<Vec<PersonaSnapshot>> {
        let canonical_target = Self::normalize_target(target_name);
        let manifest_path = self.manifest_file();
        let mut snapshots = Vec::new();

        if manifest_path.exists() {
            let file_content = fs::read_to_string(&manifest_path)?;
            for line in file_content.lines() {
                if let Ok(snap) = serde_json::from_str::<PersonaSnapshot>(line.trim()) {
                    if snap.target_file == canonical_target {
                        snapshots.push(snap);
                    }
                }
            }
        }

        // Sort descending by epoch timestamp (newest first)
        snapshots.sort_by(|a, b| b.created_at_epoch.cmp(&a.created_at_epoch));
        Ok(snapshots)
    }

    /// Safely reverts the target configuration file to a previous snapshot.
    /// Creates an automatic safety snapshot of the current state before replacing.
    pub fn rollback(&self, target_name: &str, snapshot_id: Option<&str>) -> anyhow::Result<RollbackOutcome> {
        let canonical_target = Self::normalize_target(target_name);
        let target_path = self.config_dir.join(canonical_target);
        let stem = canonical_target.strip_suffix(".md").unwrap_or(canonical_target);

        let history_dir = self.history_dir();
        let snapshots = self.list_snapshots(canonical_target)?;

        let chosen_backup_path = if let Some(id) = snapshot_id {
            // Find specific snapshot by ID or filename
            let found = snapshots.iter().find(|s| s.id == id || s.backup_filename == id);
            match found {
                Some(s) => history_dir.join(&s.backup_filename),
                None => {
                    // Check if file exists directly in history dir
                    let direct_file = history_dir.join(id);
                    if direct_file.exists() {
                        direct_file
                    } else {
                        anyhow::bail!(
                            "Snapshot ID '{}' tidak ditemukan untuk {}. Ketik `aina persona history` untuk melihat daftar.",
                            id, canonical_target
                        );
                    }
                }
            }
        } else {
            // Pick the latest snapshot that is not an auto-safety snapshot, or fall back to previous.md
            let non_safety = snapshots.iter().find(|s| !s.reason.starts_with("Auto-safety snapshot"));
            if let Some(s) = non_safety {
                history_dir.join(&s.backup_filename)
            } else if let Some(first) = snapshots.first() {
                history_dir.join(&first.backup_filename)
            } else {
                let prev = self.config_dir.join(format!("{}.previous.md", stem));
                if prev.exists() {
                    prev
                } else {
                    anyhow::bail!(
                        "Tidak ada snapshot riwayat yang ditemukan untuk {}. Tidak ada titik rollback.",
                        canonical_target
                    );
                }
            }
        };

        if !chosen_backup_path.exists() {
            anyhow::bail!(
                "Berkas backup {} fisik tidak ditemukan di disk.",
                chosen_backup_path.display()
            );
        }

        // 1. Mandatory safety snapshot of CURRENT state before rollback
        let safety_reason = format!(
            "Auto-safety snapshot before rolling back to {}",
            chosen_backup_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
        );
        let safety_snap = self.create_snapshot(canonical_target, &safety_reason)?;

        // 2. Perform atomic replace (write to temp then rename)
        let backup_content = fs::read(&chosen_backup_path)?;
        let temp_path = self.config_dir.join(format!("{}.tmp_{}", stem, rand::random::<u32>()));
        fs::write(&temp_path, &backup_content)?;
        fs::rename(&temp_path, &target_path)?;

        let restored_byte_size = backup_content.len() as u64;
        let restored_id = chosen_backup_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(RollbackOutcome {
            target_file: canonical_target.to_string(),
            restored_from_snapshot_id: restored_id,
            safety_snapshot_id: safety_snap.id,
            restored_byte_size,
        })
    }

    /// Resets the configuration file to the pristine factory default template.
    /// Creates an automatic safety snapshot before restoring.
    pub fn restore_default(&self, target_name: &str) -> anyhow::Result<RestoreDefaultOutcome> {
        let canonical_target = Self::normalize_target(target_name);
        let target_path = self.config_dir.join(canonical_target);
        let stem = canonical_target.strip_suffix(".md").unwrap_or(canonical_target);

        // Find default template path
        let candidates = [
            self.config_dir.join(format!("{}.default.md", stem)),
            self.config_dir.join(format!("{}.default", stem)),
            PathBuf::from("/app/config").join(format!("{}.default.md", stem)),
            PathBuf::from("/app/config").join(format!("{}.default", stem)),
            PathBuf::from("config").join(format!("{}.default.md", stem)),
            PathBuf::from("config").join(format!("{}.default", stem)),
        ];

        let template_path = candidates.into_iter().find(|p| p.exists());
        let template_path = match template_path {
            Some(p) => p,
            None => {
                anyhow::bail!(
                    "Template default untuk {} ({}.default.md) tidak ditemukan.",
                    canonical_target, stem
                );
            }
        };

        // 1. Mandatory safety snapshot of CURRENT state before resetting
        let safety_snap = self.create_snapshot(
            canonical_target,
            &format!("Auto-safety snapshot before factory default reset from {}", template_path.display()),
        )?;

        // 2. Perform atomic replace
        let default_content = fs::read(&template_path)?;
        let temp_path = self.config_dir.join(format!("{}.tmp_{}", stem, rand::random::<u32>()));
        fs::write(&temp_path, &default_content)?;
        fs::rename(&temp_path, &target_path)?;

        Ok(RestoreDefaultOutcome {
            target_file: canonical_target.to_string(),
            safety_snapshot_id: safety_snap.id,
            template_path: template_path.display().to_string(),
            restored_byte_size: default_content.len() as u64,
        })
    }

    /// Safely updates target file content with an automatic pre-modification snapshot.
    pub fn set_content(
        &self,
        target_name: &str,
        new_content: &str,
        reason: &str,
    ) -> anyhow::Result<PersonaSnapshot> {
        let canonical_target = Self::normalize_target(target_name);
        let target_path = self.config_dir.join(canonical_target);
        let stem = canonical_target.strip_suffix(".md").unwrap_or(canonical_target);

        // 1. Mandatory snapshot of existing state before updating
        let snapshot = if target_path.exists() {
            self.create_snapshot(
                canonical_target,
                &format!("Pre-update backup: {}", reason),
            )?
        } else {
            // If creating first time, still return a virtual snapshot info
            PersonaSnapshot {
                id: "initial".to_string(),
                target_file: canonical_target.to_string(),
                backup_filename: "none".to_string(),
                reason: reason.to_string(),
                created_at_epoch: 0,
                created_at_formatted: "N/A".to_string(),
                byte_size: 0,
                line_count: 0,
            }
        };

        // 2. Atomic write
        let temp_path = self.config_dir.join(format!("{}.tmp_{}", stem, rand::random::<u32>()));
        fs::write(&temp_path, new_content.as_bytes())?;
        fs::rename(&temp_path, &target_path)?;

        Ok(snapshot)
    }

    /// Inspects status of all supported persona and character configuration files.
    pub fn get_status(&self) -> anyhow::Result<Vec<PersonaFileStatus>> {
        let mut results = Vec::new();

        for &canonical_target in SUPPORTED_TARGETS {
            let target_path = self.config_dir.join(canonical_target);
            let stem = canonical_target.strip_suffix(".md").unwrap_or(canonical_target);
            let exists = target_path.exists();

            let byte_size = if exists {
                target_path.metadata()?.len()
            } else {
                0
            };

            let last_modified = if exists {
                target_path
                    .metadata()?
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| format_epoch_local(d.as_secs()))
            } else {
                None
            };

            let default_template_exists = self.config_dir.join(format!("{}.default.md", stem)).exists()
                || self.config_dir.join(format!("{}.default", stem)).exists()
                || PathBuf::from("config").join(format!("{}.default.md", stem)).exists();

            let snapshots = self.list_snapshots(canonical_target).unwrap_or_default();
            let snapshot_count = snapshots.len();
            let latest_snapshot_id = snapshots.first().map(|s| s.id.clone());

            results.push(PersonaFileStatus {
                target_file: canonical_target.to_string(),
                exists,
                path: target_path.display().to_string(),
                byte_size,
                last_modified,
                default_template_exists,
                snapshot_count,
                latest_snapshot_id,
            });
        }

        Ok(results)
    }
}

fn format_epoch_local(epoch: u64) -> String {
    // Offset +7 WIB
    let local_secs = (epoch as i64) + (7 * 3600);
    let days = local_secs.div_euclid(86400);
    let rem_secs = local_secs.rem_euclid(86400);

    let hh = rem_secs / 3600;
    let mm = (rem_secs % 3600) / 60;
    let ss = rem_secs % 60;

    let (y, m, d) = civil_from_days(days);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} WIB", y, m, d, hh, mm, ss)
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persona_lifecycle_snapshot_rollback_and_restore_default() {
        let temp_dir = std::env::temp_dir().join(format!("aina_test_persona_lifecycle_{}", rand::random::<u32>()));
        fs::create_dir_all(&temp_dir).unwrap();

        let mgr = PersonaLifecycleManager::new(&temp_dir);

        // 1. Create factory default template and active persona
        let default_content = "# Persona Default Aina\nVersi bawaan pabrik.";
        let custom_v1 = "# Persona Custom V1\nGaya kasual santai.";
        let custom_v2 = "# Persona Custom V2\nGaya formal profesional.";

        fs::write(temp_dir.join("persona.default.md"), default_content).unwrap();
        fs::write(temp_dir.join("persona.md"), custom_v1).unwrap();

        // 2. Create snapshot of V1
        let snap1 = mgr.create_snapshot("persona", "Versi awal santai").unwrap();
        assert_eq!(snap1.target_file, "persona.md");
        assert_eq!(snap1.reason, "Versi awal santai");

        // 3. Update to V2 (which takes pre-modification snapshot)
        let _snap2 = mgr.set_content("persona", custom_v2, "Ganti ke formal").unwrap();
        let current_text = fs::read_to_string(temp_dir.join("persona.md")).unwrap();
        assert_eq!(current_text, custom_v2);

        // 4. Verify list_snapshots
        let history = mgr.list_snapshots("persona").unwrap();
        assert!(history.len() >= 2);

        // 5. Rollback to V1
        let rollback_res = mgr.rollback("persona", Some(&snap1.id)).unwrap();
        assert_eq!(rollback_res.target_file, "persona.md");

        let rolled_back_text = fs::read_to_string(temp_dir.join("persona.md")).unwrap();
        assert_eq!(rolled_back_text, custom_v1);

        // 6. Restore to factory default
        let default_res = mgr.restore_default("persona").unwrap();
        assert_eq!(default_res.target_file, "persona.md");

        let factory_text = fs::read_to_string(temp_dir.join("persona.md")).unwrap();
        assert_eq!(factory_text, default_content);

        // 7. Check status
        let statuses = mgr.get_status().unwrap();
        let persona_status = statuses.iter().find(|s| s.target_file == "persona.md").unwrap();
        assert!(persona_status.exists);
        assert!(persona_status.default_template_exists);
        assert!(persona_status.snapshot_count >= 3);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
