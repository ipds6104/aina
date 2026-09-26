//! Model aliases and catalog management for Antigravity CLI.

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
            anyhow::bail!(
                "Model '{}' tidak didukung. Pilihan: gemini-3.8-flash-medium (default), gemini-3.8-flash-high, gemini-3.8-flash-low, gemini-3.1-pro-high, claude-opus-4-6-thinking, claude-sonnet-4-6.",
                other
            )
        }
    }
}

pub fn get_available_models() -> Vec<(&'static str, &'static str)> {
    vec![
        ("gemini-3.8-flash-medium", "Gemini 3.8 Flash (Medium) - Default Cepat & Seimbang"),
        ("gemini-3.8-flash-high", "Gemini 3.8 Flash (High) - Penalaran Tinggi / Deep Thinking"),
        ("gemini-3.8-flash-low", "Gemini 3.8 Flash (Low) - Respons Kilat & Kasual"),
        ("gemini-3.1-pro-high", "Gemini 3.1 Pro (High) - Deep Coding & Arsitektur"),
        ("claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking) - Khusus Tugas Kompleks Eksplisit"),
        ("claude-sonnet-4-6", "Claude Sonnet 4.6 (Thinking)"),
    ]
}
