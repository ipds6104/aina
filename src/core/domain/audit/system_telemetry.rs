//! Host system telemetry: memory inspection, uptime, directory sizes, and workspace git status.

use super::models::WorkspaceProvenance;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static PROCESS_START_INSTANT: OnceLock<Instant> = OnceLock::new();

/// How long a computed directory size stays valid.
///
/// The audit dashboard asks for the transcript brain size on every load, while
/// the tree itself changes slowly. Walking it eagerly meant a full recursive
/// scan of `/root/.gemini/...` per request (minutes on a large brain).
const DIR_SIZE_TTL: Duration = Duration::from_secs(30);

type DirSizeCache = Mutex<HashMap<PathBuf, (u64, Instant)>>;

fn dir_size_cache() -> &'static DirSizeCache {
    static CACHE: OnceLock<DirSizeCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_process_uptime_secs() -> u64 {
    let start = PROCESS_START_INSTANT.get_or_init(std::time::Instant::now);
    start.elapsed().as_secs()
}

pub struct SystemTelemetry;

impl SystemTelemetry {
    /// Reads resident set size (RSS) and total virtual memory (VIRT) in bytes from /proc/self/statm.
    pub fn read_process_memory() -> (u64, u64) {
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = content.split_whitespace().collect();
            if parts.len() >= 2 {
                let total_pages = parts[0].parse::<u64>().unwrap_or(0);
                let resident_pages = parts[1].parse::<u64>().unwrap_or(0);
                let page_size = 4096u64;
                return (resident_pages * page_size, total_pages * page_size);
            }
        }
        (0, 0)
    }

    /// Formats byte counts into human-readable strings (e.g. "12.45 MB").
    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * KB;
        const GB: u64 = 1024 * MB;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.2} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.2} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Computes total size of a directory in bytes recursively.
    ///
    /// Results are cached for [`DIR_SIZE_TTL`] so polling the dashboard does not
    /// re-walk a large transcript tree on every request.
    pub fn compute_dir_size<P: AsRef<Path>>(path: P) -> u64 {
        let path = path.as_ref();
        let cache_key = path.to_path_buf();

        if let Ok(cache) = dir_size_cache().lock() {
            if let Some((size, computed_at)) = cache.get(&cache_key) {
                if computed_at.elapsed() < DIR_SIZE_TTL {
                    return *size;
                }
            }
        }

        // Walk without holding the lock: a large tree can take a long time and
        // must not block other callers.
        let size = Self::walk_dir_size(path);

        if let Ok(mut cache) = dir_size_cache().lock() {
            cache.insert(cache_key, (size, Instant::now()));
        }

        size
    }

    fn walk_dir_size(path: &Path) -> u64 {
        let mut total = 0u64;
        let mut stack = vec![path.to_path_buf()];

        while let Some(current) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_symlink() {
                        continue;
                    }
                    if p.is_dir() {
                        stack.push(p);
                    } else if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
            }
        }
        total
    }

    /// Inspects workspaces and extracts git repository provenance and script count.
    pub fn inspect_workspaces<P: AsRef<Path>>(workspace_dir: P) -> Vec<WorkspaceProvenance> {
        let mut provenances = Vec::new();
        let ws_path = workspace_dir.as_ref();

        // 1. Inspect the main workspace directory
        if ws_path.is_dir() {
            let name = ws_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "default".to_string());
            let (is_git, commit, branch, is_dirty) = Self::get_git_info_for_dir(ws_path);
            let scripts_count = ws_path
                .join("scripts")
                .read_dir()
                .map(|e| e.flatten().filter(|f| f.path().is_file()).count())
                .unwrap_or(0);

            provenances.push(WorkspaceProvenance {
                name,
                path: ws_path.to_string_lossy().to_string(),
                is_git_repo: is_git,
                git_commit: commit,
                git_branch: branch,
                is_dirty,
                scripts_count,
            });

            // 2. If workspace_dir is inside a parent `workspaces` directory, check sibling workspaces
            if let Some(parent) = ws_path.parent() {
                if parent.is_dir() && parent.file_name().map(|n| n == "workspaces").unwrap_or(false) {
                    if let Ok(siblings) = std::fs::read_dir(parent) {
                        for entry in siblings.flatten() {
                            let p = entry.path();
                            if p.is_dir() && p != ws_path {
                                let sib_name = p
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "sibling".to_string());
                                let (s_git, s_commit, s_branch, s_dirty) = Self::get_git_info_for_dir(&p);
                                let s_scripts = p
                                    .join("scripts")
                                    .read_dir()
                                    .map(|e| e.flatten().filter(|f| f.path().is_file()).count())
                                    .unwrap_or(0);

                                provenances.push(WorkspaceProvenance {
                                    name: sib_name,
                                    path: p.to_string_lossy().to_string(),
                                    is_git_repo: s_git,
                                    git_commit: s_commit,
                                    git_branch: s_branch,
                                    is_dirty: s_dirty,
                                    scripts_count: s_scripts,
                                });
                            }
                        }
                    }
                }
            }
        }

        provenances
    }

    /// Gets git commit, branch, and dirty status for a directory.
    pub fn get_git_info_for_dir<P: AsRef<Path>>(dir: P) -> (bool, Option<String>, Option<String>, bool) {
        let p = dir.as_ref();
        let has_git_folder = p.join(".git").exists();

        let commit_out = std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output();

        let commit = commit_out.ok().and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            } else {
                None
            }
        });

        let branch_out = std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .output();

        let branch = branch_out.ok().and_then(|o| {
            if o.status.success() {
                let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if s.is_empty() {
                    None
                } else {
                    Some(s)
                }
            } else {
                None
            }
        });

        let status_out = std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .arg("status")
            .arg("--porcelain")
            .output();

        let is_dirty = status_out
            .ok()
            .map(|o| o.status.success() && !o.stdout.is_empty())
            .unwrap_or(false);

        let is_git = has_git_folder || commit.is_some();
        (is_git, commit, branch, is_dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aina_telemetry_{}_{}_{}",
            tag,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn compute_dir_size_sums_nested_files() {
        let dir = unique_dir("size");
        fs::write(dir.join("a.bin"), vec![0u8; 100]).unwrap();
        let nested = dir.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("b.bin"), vec![0u8; 50]).unwrap();

        assert_eq!(SystemTelemetry::compute_dir_size(&dir), 150);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn compute_dir_size_is_cached_within_ttl() {
        let dir = unique_dir("cache");
        fs::write(dir.join("a.bin"), vec![0u8; 10]).unwrap();

        assert_eq!(SystemTelemetry::compute_dir_size(&dir), 10);

        // A second file inside the TTL window must not trigger a re-walk.
        fs::write(dir.join("b.bin"), vec![0u8; 90]).unwrap();
        assert_eq!(SystemTelemetry::compute_dir_size(&dir), 10);
    }

    #[test]
    fn compute_dir_size_of_missing_path_is_zero() {
        let missing = std::env::temp_dir().join("aina_telemetry_missing_does_not_exist");
        assert_eq!(SystemTelemetry::compute_dir_size(&missing), 0);
    }
}
