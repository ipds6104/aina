//! Model aliases and catalog management for Antigravity CLI.
//!
//! # Dynamic Model Discovery
//! `fetch_models_from_agy_cli()` calls `agy models` at runtime and parses the
//! tab-separated output into `DynamicModelEntry` structs. Falls back to the
//! static catalog if the CLI is unreachable or returns unexpected output.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// A single model entry returned by the dynamic discovery endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicModelEntry {
    pub id: String,
    pub name: String,
    /// Inferred category: "gemini" | "claude" | "gpt" | "other"
    pub category: String,
}

impl DynamicModelEntry {
    fn infer_category(id: &str) -> String {
        let lower = id.to_lowercase();
        if lower.contains("gemini") {
            "gemini".into()
        } else if lower.contains("claude") {
            "claude".into()
        } else if lower.contains("gpt") {
            "gpt".into()
        } else {
            "other".into()
        }
    }
}

/// Static fallback catalog — used when `agy models` is unavailable.
fn static_model_catalog() -> Vec<DynamicModelEntry> {
    vec![
        ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium) — Default Cepat & Seimbang"),
        ("gemini-3.8-flash-high",   "Gemini 3.8 Flash (High) — Penalaran Tinggi / Deep Thinking"),
        ("gemini-3.8-flash-low",    "Gemini 3.8 Flash (Low) — Respons Kilat & Kasual"),
        ("gemini-3.1-pro-high",     "Gemini 3.1 Pro (High) — Deep Coding & Arsitektur"),
        ("claude-sonnet-4-6",       "Claude Sonnet 4.6 (Thinking)"),
        ("claude-opus-4-6-thinking","Claude Opus 4.6 (Thinking) — Khusus Tugas Kompleks"),
        ("gpt-oss-120b-medium",     "GPT-OSS 120B (Medium)"),
    ]
    .into_iter()
    .map(|(id, name)| {
        let category = DynamicModelEntry::infer_category(id);
        DynamicModelEntry { id: id.into(), name: name.into(), category }
    })
    .collect()
}

/// Parses a single line of `agy models` tab-separated output.
/// Format: `<model-id>\t<Display Name>\n`
/// Lines that don't contain a tab are silently skipped (e.g. "Fetching available models...")
fn parse_agy_models_line(line: &str) -> Option<DynamicModelEntry> {
    let parts: Vec<&str> = line.splitn(2, '\t').collect();
    if parts.len() == 2 {
        let id = parts[0].trim().to_string();
        let name = parts[1].trim().to_string();
        if id.is_empty() || name.is_empty() {
            return None;
        }
        let category = DynamicModelEntry::infer_category(&id);
        Some(DynamicModelEntry { id, name, category })
    } else {
        None
    }
}

/// The source of a dynamic discovery result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    /// Parsed live from the `agy models` CLI output.
    Dynamic,
    /// Static catalog used because the CLI failed, timed out, or
    /// returned too few valid entries.
    StaticFallback,
}

/// Result of a dynamic model discovery call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicModelDiscovery {
    pub models: Vec<DynamicModelEntry>,
    pub source: ModelSource,
}

/// Calls `agy models` as a subprocess and parses the tab-separated output.
/// Falls back to the static catalog (reported via `DynamicModelDiscovery::source`)
/// if the CLI is unavailable, times out, or produces fewer than 2 valid entries
/// (guards against partial output).
pub async fn fetch_models_from_agy_cli() -> DynamicModelDiscovery {
    let agy_bin = std::env::var("AGY_BIN").unwrap_or_else(|_| "/usr/local/bin/agy".to_string());

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        tokio::process::Command::new(&agy_bin)
            .arg("models")
            .output(),
    )
    .await;

    match result {
        Ok(Ok(output)) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let entries: Vec<DynamicModelEntry> = stdout
                .lines()
                .filter_map(parse_agy_models_line)
                .collect();

            if entries.len() >= 2 {
                info!("Dynamic model list fetched from AGY CLI: {} models", entries.len());
                DynamicModelDiscovery { models: entries, source: ModelSource::Dynamic }
            } else {
                warn!("AGY CLI returned <2 valid model lines — using static catalog");
                static_catalog_fallback()
            }
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("agy models exited non-zero ({}): {}", output.status, stderr.trim());
            static_catalog_fallback()
        }
        Ok(Err(e)) => {
            warn!("Failed to spawn agy models: {}", e);
            static_catalog_fallback()
        }
        Err(_) => {
            warn!("agy models timed out after 8s — using static catalog");
            static_catalog_fallback()
        }
    }
}

/// Wraps `static_model_catalog()` in a `DynamicModelDiscovery` with the
/// `StaticFallback` source marker.
fn static_catalog_fallback() -> DynamicModelDiscovery {
    DynamicModelDiscovery { models: static_model_catalog(), source: ModelSource::StaticFallback }
}

// ---------------------------------------------------------------------------
// Legacy sync helpers (kept for backward-compat with resolve_model_name use)
// ---------------------------------------------------------------------------

/// Normalizes and resolves model name aliases, strictly prioritizing Gemini as default
/// and allowing Claude Opus ONLY when explicitly requested.
pub fn resolve_model_name(raw: &str) -> anyhow::Result<String> {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        // Gemini 3.8 Family (Recommended Defaults)
        "gemini-3.8-flash-medium" | "flash-medium" | "medium" | "gemini-medium" | "gemini-flash-medium" | "default" => {
            Ok("gemini-3.8-flash-medium".to_string())
        }
        "gemini-3.8-flash-high" | "flash-high" | "high" | "gemini-high" | "gemini-flash-high" => {
            Ok("gemini-3.8-flash-high".to_string())
        }
        "gemini-3.8-flash-low" | "flash-low" | "low" | "gemini-low" | "gemini-flash-low" => {
            Ok("gemini-3.8-flash-low".to_string())
        }
        "flash" | "gemini-flash" => {
            Ok("gemini-3.8-flash-medium".to_string())
        }

        // Gemini 3.7 Family
        "gemini-3.7-flash-high" => Ok("gemini-3.7-flash-high".to_string()),
        "gemini-3.7-flash-medium" => Ok("gemini-3.7-flash-medium".to_string()),
        "gemini-3.7-flash-low" => Ok("gemini-3.7-flash-low".to_string()),

        // Gemini 3.6 Family
        "gemini-3.6-flash-high" => Ok("gemini-3.6-flash-high".to_string()),
        "gemini-3.6-flash-medium" => Ok("gemini-3.6-flash-medium".to_string()),
        "gemini-3.6-flash-low" => Ok("gemini-3.6-flash-low".to_string()),

        // Gemini 3.1 Pro (Deep Coding & Architecture)
        "gemini-3.1-pro-high" | "pro-high" | "pro" | "gemini-pro" => {
            Ok("gemini-3.1-pro-high".to_string())
        }
        "gemini-3.1-pro-low" | "pro-low" => {
            Ok("gemini-3.1-pro-low".to_string())
        }

        // Claude Sonnet
        "claude-sonnet-4-6" | "claude-sonnet" | "sonnet" => {
            Ok("claude-sonnet-4-6".to_string())
        }

        // Claude Opus (Strictly opt-in / explicitly requested)
        "claude-opus-4-6-thinking" | "claude-opus" | "opus" | "opus-thinking" => {
            Ok("claude-opus-4-6-thinking".to_string())
        }

        // GPT-OSS
        "gpt-oss-120b-medium" | "gpt-oss" => {
            Ok("gpt-oss-120b-medium".to_string())
        }

        other => {
            // Accept any model ID not in the explicit deny list (forward-compat)
            if !other.is_empty() && other.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
                Ok(raw.trim().to_string())
            } else {
                anyhow::bail!(
                    "Model '{}' mengandung karakter tidak valid.",
                    other
                )
            }
        }
    }
}

/// Legacy sync accessor — returns static (id, display_name) tuples.
/// Prefer `fetch_models_from_agy_cli()` for dynamic discovery.
pub fn get_available_models() -> Vec<(&'static str, &'static str)> {
    vec![
        ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium) - Default Cepat & Seimbang"),
        ("gemini-3.8-flash-high",   "Gemini 3.8 Flash (High) - Penalaran Tinggi / Deep Thinking"),
        ("gemini-3.8-flash-low",    "Gemini 3.8 Flash (Low) - Respons Kilat & Kasual"),
        ("gemini-3.1-pro-high",     "Gemini 3.1 Pro (High) - Deep Coding & Arsitektur"),
        ("claude-opus-4-6-thinking","Claude Opus 4.6 (Thinking) - Khusus Tugas Kompleks Eksplisit"),
        ("claude-sonnet-4-6",       "Claude Sonnet 4.6 (Thinking)"),
    ]
}
