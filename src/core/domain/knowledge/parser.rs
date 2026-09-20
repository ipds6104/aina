use std::collections::HashMap;

/// Pure YAML & Markdown Frontmatter Parser (Zero I/O)
pub struct FrontmatterParser;

impl FrontmatterParser {
    /// Extracts YAML frontmatter between `---` markers and body
    pub fn parse(content: &str) -> (Option<HashMap<String, serde_yaml::Value>>, String) {
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
}
