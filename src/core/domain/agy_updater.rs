//! Pure hexagonal domain logic for Antigravity CLI (`agy`) version introspection,
//! contract verification, and safe canary-gated update decisions (ADR-001).
//!
//! Zero async runtime, zero HTTP client, zero filesystem I/O inside this module.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// CLI flags that `AntigravityCliAdapter` strictly depends on at runtime.
/// If a future `agy` release removes or renames any of these flags, the canary
/// gate MUST reject the update before replacing the production binary.
pub const REQUIRED_AGY_CLI_FLAGS: &[&str] = &[
    "--output-format",
    "--print-timeout",
    "--dangerously-skip-permissions",
    "--conversation",
    "--model",
    "--effort",
];

/// Parsed semantic version (`MAJOR.MINOR.PATCH`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemVer {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl SemVer {
    /// Extracts the first `X.Y.Z` semantic version triplet from arbitrary text
    /// such as `"1.5.13"`, `"v1.5.13"`, `"agy 1.5.13\n"`, or `"1.5.13-5999302598852608"`.
    pub fn parse(raw: &str) -> Option<Self> {
        for token in raw.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
            let trimmed = token.trim().trim_start_matches(['v', 'V']);
            let core = trimmed.split(['-', '+']).next().unwrap_or("");
            let mut parts = core.split('.');
            let major = parts.next()?.parse::<u64>().ok();
            let minor = parts.next().and_then(|s| s.parse::<u64>().ok());
            let patch = parts.next().and_then(|s| s.parse::<u64>().ok());
            if let (Some(major), Some(minor), Some(patch)) = (major, minor, patch) {
                if parts.next().is_none() {
                    return Some(Self { major, minor, patch });
                }
            }
        }
        None
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for SemVer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Official Antigravity CLI release manifest JSON structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgyManifest {
    pub version: String,
    pub url: String,
    pub sha512: String,
}

/// Deterministic comparison result between installed and upstream manifest versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdatePolicyDecision {
    UpToDate {
        installed: String,
        latest: String,
    },
    UpdateAvailable {
        installed: String,
        latest: String,
    },
    InstalledNewerThanManifest {
        installed: String,
        latest: String,
    },
    UnknownVersion {
        installed: Option<String>,
        latest: Option<String>,
    },
}

/// Evaluates whether an upgrade is available using strict semantic version comparison.
pub fn evaluate_version_status(
    installed_raw: Option<&str>,
    latest_raw: Option<&str>,
) -> UpdatePolicyDecision {
    let installed_ver = installed_raw.and_then(SemVer::parse);
    let latest_ver = latest_raw.and_then(SemVer::parse);

    match (installed_ver, latest_ver) {
        (Some(inst), Some(lat)) => match inst.cmp(&lat) {
            Ordering::Less => UpdatePolicyDecision::UpdateAvailable {
                installed: inst.to_string(),
                latest: lat.to_string(),
            },
            Ordering::Equal => UpdatePolicyDecision::UpToDate {
                installed: inst.to_string(),
                latest: lat.to_string(),
            },
            Ordering::Greater => UpdatePolicyDecision::InstalledNewerThanManifest {
                installed: inst.to_string(),
                latest: lat.to_string(),
            },
        },
        _ => UpdatePolicyDecision::UnknownVersion {
            installed: installed_ver.map(|v| v.to_string()).or_else(|| {
                installed_raw
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            }),
            latest: latest_ver.map(|v| v.to_string()).or_else(|| {
                latest_raw
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            }),
        },
    }
}

/// Maps runtime OS + CPU architecture + libc flavor to the official Antigravity manifest platform key.
pub fn detect_manifest_platform(os: &str, arch: &str, is_musl: bool) -> Option<&'static str> {
    match (os, arch, is_musl) {
        ("linux", "x86_64", false) => Some("linux_amd64"),
        ("linux", "x86_64", true) => Some("linux_amd64_musl"),
        ("linux", "aarch64", false) => Some("linux_arm64"),
        ("linux", "aarch64", true) => Some("linux_arm64_musl"),
        ("macos", "aarch64", _) => Some("darwin_arm64"),
        ("macos", "x86_64", _) => Some("darwin_amd64"),
        _ => None,
    }
}

/// Canary Gate 1: Verifies that `agy --help` still supports every CLI flag required by Aina.
pub fn validate_agy_help_contract(help_output: &str) -> Result<(), Vec<String>> {
    let missing: Vec<String> = REQUIRED_AGY_CLI_FLAGS
        .iter()
        .filter(|&&flag| !help_output.contains(flag))
        .map(|s| (*s).to_string())
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(missing)
    }
}

/// Minimal contract matching `AgyJsonOutput` expected by `AntigravityCliAdapter`.
#[derive(Debug, Deserialize)]
struct CanaryJsonPayload {
    pub conversation_id: String,
    pub status: String,
    #[serde(default)]
    pub response: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// Canary Gate 2: Validates that a live `agy -p ... --output-format json` stdout
/// produces a parseable JSON line with non-error status and non-empty response.
pub fn validate_canary_json_response(stdout: &str) -> Result<String, String> {
    for line in stdout.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            if let Ok(parsed) = serde_json::from_str::<CanaryJsonPayload>(trimmed) {
                if parsed.status.eq_ignore_ascii_case("error") {
                    return Err(format!(
                        "Canary JSON returned error status: {}",
                        parsed.error.unwrap_or_else(|| "unknown error".to_string())
                    ));
                }
                if parsed.conversation_id.trim().is_empty() {
                    return Err("Canary JSON missing conversation_id".to_string());
                }
                if parsed.response.trim().is_empty() {
                    return Err("Canary JSON returned empty response".to_string());
                }
                return Ok(parsed.response.trim().to_string());
            }
        }
    }
    Err("Canary stdout did not contain a valid Antigravity JSON response object".to_string())
}

/// Extracts the latest release notes section under `## Antigravity CLI` from the
/// official changelog Markdown (`https://antigravity.google/docs/changelog.md`).
pub fn extract_cli_changelog_excerpt(changelog_md: &str, max_lines: usize) -> Option<String> {
    let mut in_cli_section = false;
    let mut collected = Vec::new();
    let mut release_headings_seen = 0usize;

    for line in changelog_md.lines() {
        let trimmed = line.trim();
        if !in_cli_section {
            if trimmed.eq_ignore_ascii_case("## Antigravity CLI") {
                in_cli_section = true;
            }
            continue;
        }

        // Stop when the next top-level product section starts (e.g. `## Antigravity IDE`)
        if trimmed.starts_with("## ") {
            break;
        }
        // Keep only the newest release subsection (`### ...`)
        if trimmed.starts_with("### ") {
            release_headings_seen += 1;
            if release_headings_seen > 1 {
                break;
            }
        }
        if !trimmed.is_empty() || !collected.is_empty() {
            collected.push(line);
            if collected.len() >= max_lines.max(1) {
                break;
            }
        }
    }

    let excerpt = collected.join("\n").trim().to_string();
    if excerpt.is_empty() {
        None
    } else {
        Some(excerpt)
    }
}

/// Constant-time hex digest comparison (case-insensitive) for SHA-512 manifest verification.
pub fn constant_time_hex_eq(a: &str, b: &str) -> bool {
    let a_bytes = a.trim().to_ascii_lowercase();
    let b_bytes = b.trim().to_ascii_lowercase();
    if a_bytes.is_empty() || a_bytes.len() != b_bytes.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a_bytes.bytes().zip(b_bytes.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Public-safe summary embedded in `GET /api/status` (no internal filesystem paths leaked).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgyCliPublicSummary {
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub auto_update_disabled: bool,
    pub auto_canary_enabled: bool,
    pub checked_at_epoch: i64,
}

/// Full operator health report exposed via authenticated `GET /api/agy/status` and `aina agy status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgyCliHealthReport {
    pub binary_path: String,
    pub canonical_path: String,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub decision: UpdatePolicyDecision,
    pub auto_update_disabled: bool,
    pub auto_canary_enabled: bool,
    pub has_rollback_backup: bool,
    pub local_updater_status: Option<String>,
    pub manifest_error: Option<String>,
    pub checked_at_epoch: i64,
}

impl AgyCliHealthReport {
    pub fn to_public_summary(&self) -> AgyCliPublicSummary {
        AgyCliPublicSummary {
            installed_version: self.installed_version.clone(),
            latest_version: self.latest_version.clone(),
            update_available: self.update_available,
            auto_update_disabled: self.auto_update_disabled,
            auto_canary_enabled: self.auto_canary_enabled,
            checked_at_epoch: self.checked_at_epoch,
        }
    }
}

/// Structured outcome of `aina agy update` or `aina agy rollback`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgyUpdateResult {
    pub ok: bool,
    pub status: String,
    pub previous_version: Option<String>,
    pub new_version: Option<String>,
    pub sha512_verified: bool,
    pub canary_help_passed: bool,
    pub canary_llm_passed: Option<bool>,
    pub backup_path: Option<String>,
    pub changelog_excerpt: Option<String>,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parses_all_cli_formats_and_orders_correctly() {
        assert_eq!(SemVer::parse("1.5.13"), Some(SemVer { major: 1, minor: 5, patch: 13 }));
        assert_eq!(SemVer::parse("v1.5.10\n"), Some(SemVer { major: 1, minor: 5, patch: 10 }));
        assert_eq!(SemVer::parse("agy 1.5.14-build123"), Some(SemVer { major: 1, minor: 5, patch: 14 }));
        assert_eq!(SemVer::parse("not-a-version"), None);
        assert!(SemVer::parse("1.5.13").unwrap() > SemVer::parse("1.5.10").unwrap());
        assert!(SemVer::parse("1.10.0").unwrap() > SemVer::parse("1.5.99").unwrap());
    }

    #[test]
    fn evaluate_version_status_detects_updates_and_prevents_downgrade() {
        assert_eq!(
            evaluate_version_status(Some("1.5.10"), Some("1.5.13")),
            UpdatePolicyDecision::UpdateAvailable {
                installed: "1.5.10".into(),
                latest: "1.5.13".into(),
            }
        );
        assert_eq!(
            evaluate_version_status(Some("1.5.13"), Some("1.5.13")),
            UpdatePolicyDecision::UpToDate {
                installed: "1.5.13".into(),
                latest: "1.5.13".into(),
            }
        );
        assert_eq!(
            evaluate_version_status(Some("1.5.14"), Some("1.5.13")),
            UpdatePolicyDecision::InstalledNewerThanManifest {
                installed: "1.5.14".into(),
                latest: "1.5.13".into(),
            }
        );
    }

    #[test]
    fn validate_agy_help_contract_catches_missing_required_flags() {
        let valid_help = "Options:\n  --output-format\n  --print-timeout\n  --dangerously-skip-permissions\n  --conversation\n  --model\n  --effort\n";
        assert!(validate_agy_help_contract(valid_help).is_ok());

        let broken_help = "Options:\n  --model\n  --conversation\n";
        let missing = validate_agy_help_contract(broken_help).unwrap_err();
        assert!(missing.contains(&"--output-format".to_string()));
        assert!(missing.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn validate_canary_json_response_enforces_schema_and_rejects_errors() {
        let ok_stdout = "debug line\n{\"conversation_id\":\"c1\",\"status\":\"DONE\",\"response\":\"OK\"}\n";
        assert_eq!(validate_canary_json_response(ok_stdout).unwrap(), "OK");

        let err_stdout = "{\"conversation_id\":\"c1\",\"status\":\"error\",\"error\":\"invalid flag\"}\n";
        assert!(validate_canary_json_response(err_stdout).is_err());

        let empty_stdout = "plain text without json";
        assert!(validate_canary_json_response(empty_stdout).is_err());
    }

    #[test]
    fn extract_cli_changelog_excerpt_reads_only_latest_cli_release() {
        let md = "## Antigravity App\n### 2.0.0\n- App stuff\n## Antigravity CLI\n### 1.5.14\n- Faster streaming\n- Fix timeout\n### 1.5.13\n- Older release\n## Antigravity IDE\n- IDE stuff\n";
        let excerpt = extract_cli_changelog_excerpt(md, 10).unwrap();
        assert!(excerpt.contains("1.5.14"));
        assert!(excerpt.contains("Faster streaming"));
        assert!(!excerpt.contains("1.5.13"));
        assert!(!excerpt.contains("IDE stuff"));
    }

    #[test]
    fn constant_time_hex_eq_checks_digests_case_insensitively() {
        assert!(constant_time_hex_eq("abcdef01", "ABCDEF01"));
        assert!(!constant_time_hex_eq("abcdef01", "abcdef02"));
        assert!(!constant_time_hex_eq("", ""));
    }
}
