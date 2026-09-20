use super::parser::FrontmatterParser;
use super::{ActivityMetadata, UniversalDoc};
use std::path::Path;

/// Filesystem Document Discovery & Traversal Engine
pub struct DocumentScanner;

impl DocumentScanner {
    /// Detect if string matches a standard period pattern (e.g. 2026, 2026-09, 2026-Q1)
    pub fn is_period(s: &str) -> bool {
        (s.len() == 4 && s.chars().all(|c| c.is_ascii_digit()))
            || (s.len() == 7
                && s.chars().take(4).all(|c| c.is_ascii_digit())
                && &s[4..5] == "-"
                && s[5..].chars().all(|c| c.is_ascii_digit()))
            || (s.len() == 7 && s.chars().take(4).all(|c| c.is_ascii_digit()) && &s[4..6] == "-Q")
    }

    /// Scan universal documentation in `knowledge/universal/*.md` and root `facts.md`, `procedures.md`
    pub fn scan_universal<P: AsRef<Path>>(workspace_dir: P) -> Vec<UniversalDoc> {
        let ws = workspace_dir.as_ref();
        let knowledge_dir = ws.join("knowledge");
        if !knowledge_dir.is_dir() {
            return Vec::new();
        }

        let mut docs = Vec::new();
        let mut candidates = Vec::new();

        // 1. Files in knowledge/universal/
        let uni_dir = knowledge_dir.join("universal");
        if uni_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&uni_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                        candidates.push(path);
                    }
                }
            }
        }

        // 2. Standard top-level knowledge files
        for fname in &["facts.md", "procedures.md", "guidelines.md"] {
            let p = knowledge_dir.join(fname);
            if p.is_file() {
                candidates.push(p);
            }
        }

        for path in candidates {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            if let Ok(content) = std::fs::read_to_string(&path) {
                let (fm, body) = FrontmatterParser::parse(&content);

                let title = fm
                    .as_ref()
                    .and_then(|m| m.get("title").or_else(|| m.get("nama")))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| {
                        body.lines()
                            .find(|l| l.starts_with("# "))
                            .map(|l| l[2..].trim().to_string())
                            .unwrap_or_else(|| filename.clone())
                    });

                let summary = fm
                    .as_ref()
                    .and_then(|m| m.get("summary").or_else(|| m.get("deskripsi")))
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| {
                        body.lines()
                            .filter(|l| !l.is_empty() && !l.starts_with('#'))
                            .take(2)
                            .collect::<Vec<_>>()
                            .join(" ")
                    });

                let rel_path = path
                    .strip_prefix(ws)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| format!("knowledge/{}", filename));

                docs.push(UniversalDoc {
                    filename,
                    title,
                    summary,
                    path: rel_path,
                });
            }
        }

        docs.sort_by(|a, b| a.filename.cmp(&b.filename));
        docs
    }

    /// Scan activities across `knowledge/kegiatan/` and `knowledge/activities/`
    pub fn scan_activities<P: AsRef<Path>>(workspace_dir: P) -> Vec<ActivityMetadata> {
        let ws = workspace_dir.as_ref();
        let mut activities = Vec::new();

        let base_dirs = [
            ws.join("knowledge").join("kegiatan"),
            ws.join("knowledge").join("activities"),
            ws.join("kegiatan"),
        ];

        for base in base_dirs {
            if !base.is_dir() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&base) {
                for entry1 in entries.flatten() {
                    let p1 = entry1.path();
                    if !p1.is_dir() || p1.file_name().unwrap().to_string_lossy().starts_with('.') {
                        continue;
                    }

                    if let Ok(sub_entries) = std::fs::read_dir(&p1) {
                        for entry2 in sub_entries.flatten() {
                            let p2 = entry2.path();
                            if !p2.is_dir() || p2.file_name().unwrap().to_string_lossy().starts_with('.') {
                                continue;
                            }

                            // Try finding README.md or index.md
                            let doc_file = if p2.join("README.md").is_file() {
                                Some(p2.join("README.md"))
                            } else if p2.join("index.md").is_file() {
                                Some(p2.join("index.md"))
                            } else {
                                None
                            };

                            if let Some(doc_path) = doc_file {
                                let name1 = p1.file_name().unwrap().to_string_lossy().to_string();
                                let name2 = p2.file_name().unwrap().to_string_lossy().to_string();

                                // Detect which one is period vs slug
                                let (periode, slug) = if Self::is_period(&name2) {
                                    (name2, name1)
                                } else {
                                    (name1, name2)
                                };

                                if let Ok(content) = std::fs::read_to_string(&doc_path) {
                                    let (fm, _) = FrontmatterParser::parse(&content);
                                    let rel_path = doc_path
                                        .strip_prefix(ws)
                                        .map(|p| p.to_string_lossy().to_string())
                                        .unwrap_or_else(|_| doc_path.to_string_lossy().to_string());

                                    let act = if let Some(m) = fm {
                                        // Check single deadline or deadlines list
                                        let deadline_str = m
                                            .get("deadline")
                                            .and_then(|v| v.as_str())
                                            .map(String::from)
                                            .or_else(|| {
                                                m.get("deadlines").and_then(|v| v.as_sequence()).and_then(|seq| {
                                                    seq.first().and_then(|item| {
                                                        item.get("tanggal")
                                                            .or_else(|| item.get("date"))
                                                            .or_else(|| item.get("deadline"))
                                                            .and_then(|d| d.as_str())
                                                            .map(String::from)
                                                    })
                                                })
                                            });

                                        ActivityMetadata {
                                            nama: m
                                                .get("nama")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(&slug)
                                                .to_string(),
                                            periode: m
                                                .get("periode")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or(&periode)
                                                .to_string(),
                                            status: m
                                                .get("status")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("active")
                                                .to_string(),
                                            pic: m
                                                .get("pic")
                                                .or_else(|| m.get("peran"))
                                                .and_then(|v| v.as_str())
                                                .map(String::from),
                                            deadline: deadline_str,
                                            kategori: m
                                                .get("kategori")
                                                .and_then(|v| v.as_str())
                                                .map(String::from),
                                            path: Some(rel_path),
                                        }
                                    } else {
                                        ActivityMetadata {
                                            nama: slug.replace('-', " "),
                                            periode: periode.clone(),
                                            status: "active".to_string(),
                                            pic: None,
                                            deadline: None,
                                            kategori: None,
                                            path: Some(rel_path),
                                        }
                                    };

                                    activities.push(act);
                                }
                            }
                        }
                    }
                }
            }
        }

        activities.sort_by(|a, b| {
            b.periode
                .cmp(&a.periode)
                .then_with(|| a.nama.cmp(&b.nama))
        });
        activities
    }
}
