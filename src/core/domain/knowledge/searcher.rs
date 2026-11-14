use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSearchResult {
    pub file_path: String,
    pub relative_path: String,
    pub section_heading: Option<String>,
    pub line_number: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSearchReport {
    pub query: String,
    pub total_matches: usize,
    pub files_scanned: usize,
    pub duration_ms: u128,
    pub results: Vec<KnowledgeSearchResult>,
}

pub struct DocumentSearcher;

impl DocumentSearcher {
    pub fn search<P: AsRef<Path>>(workspace_dir: P, query: &str) -> KnowledgeSearchReport {
        let start = Instant::now();
        let ws = workspace_dir.as_ref();
        let query_lower = query.trim().to_lowercase();

        let kb_dir = ws.join("knowledge");
        let target_dir = if kb_dir.is_dir() { kb_dir } else { ws.to_path_buf() };

        let mut results = Vec::new();
        let mut files_scanned = 0;

        if !query_lower.is_empty() && target_dir.is_dir() {
            Self::scan_dir(&target_dir, ws, &query_lower, &mut results, &mut files_scanned);
        }

        let duration_ms = start.elapsed().as_millis();
        let total_matches = results.len();

        KnowledgeSearchReport {
            query: query.to_string(),
            total_matches,
            files_scanned,
            duration_ms,
            results,
        }
    }

    fn scan_dir(
        dir: &Path,
        root_ws: &Path,
        query: &str,
        results: &mut Vec<KnowledgeSearchResult>,
        files_scanned: &mut usize,
    ) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !dir_name.starts_with('.') && dir_name != "target" && dir_name != "node_modules" {
                        Self::scan_dir(&path, root_ws, query, results, files_scanned);
                    }
                } else if path.is_file() {
                    let is_md = path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("md")).unwrap_or(false);
                    if is_md {
                        *files_scanned += 1;
                        Self::scan_file(&path, root_ws, query, results);
                    }
                }
            }
        }
    }

    fn scan_file(
        file_path: &Path,
        root_ws: &Path,
        query: &str,
        results: &mut Vec<KnowledgeSearchResult>,
    ) {
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let rel_path = file_path
                .strip_prefix(root_ws)
                .unwrap_or(file_path)
                .to_string_lossy()
                .to_string();

            let mut current_heading: Option<String> = None;

            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();

                // Track heading context (#, ##, ###)
                if trimmed.starts_with('#') {
                    let heading_text = trimmed.trim_start_matches('#').trim();
                    if !heading_text.is_empty() {
                        current_heading = Some(heading_text.to_string());
                    }
                }

                if trimmed.to_lowercase().contains(query) {
                    results.push(KnowledgeSearchResult {
                        file_path: file_path.to_string_lossy().to_string(),
                        relative_path: rel_path.clone(),
                        section_heading: current_heading.clone(),
                        line_number: idx + 1,
                        snippet: trimmed.to_string(),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_document_searcher_finds_query_and_heading() {
        let temp_dir = std::env::temp_dir().join(format!(
            "aina_test_kb_search_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let kb_dir = temp_dir.join("knowledge");
        fs::create_dir_all(&kb_dir).unwrap();

        let doc1 = kb_dir.join("procedures.md");
        fs::write(
            &doc1,
            "# Standard Operating Procedures\n\n## SOP 1: Pengadaan Barang\nLangkah-langkah pengadaan barang operasional kantor.\n\n## SOP 2: Cuti Tahunan\nPengajuan cuti tahunan diajukan minimal H-3.\n",
        )
        .unwrap();

        let report = DocumentSearcher::search(&temp_dir, "cuti tahunan");
        assert_eq!(report.total_matches, 2);
        assert_eq!(report.files_scanned, 1);
        assert_eq!(
            report.results[0].section_heading.as_deref(),
            Some("SOP 2: Cuti Tahunan")
        );
        assert_eq!(report.results[0].line_number, 6);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

