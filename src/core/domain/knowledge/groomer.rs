use super::scanner::DocumentScanner;
use std::path::Path;
use tracing::info;

/// Deterministic Knowledge Base Catalog Compiler
pub struct CatalogGroomer;

impl CatalogGroomer {
    /// Compiles `knowledge/index.md` deterministically
    pub fn groom_catalog<P: AsRef<Path>>(workspace_dir: P) -> anyhow::Result<String> {
        let ws = workspace_dir.as_ref();
        let ws_name = ws.file_name().unwrap_or_default().to_string_lossy();
        let knowledge_dir = ws.join("knowledge");
        std::fs::create_dir_all(&knowledge_dir)?;

        let universal_docs = DocumentScanner::scan_universal(ws);
        let activities = DocumentScanner::scan_activities(ws);
        let archives = crate::core::domain::archive::ArchiveEngine::get_all_stats(ws)
            .unwrap_or_default();

        let mut out = String::new();
        out.push_str(&format!("# Knowledge Base Catalog - {}\n\n", ws_name));
        out.push_str("> Katalog terpadu dokumentasi universal, matriks aktivitas kerja, dan arsip obrolan. Dokumen ini digenerate secara otomatis oleh sistem Aina Engine.\n\n");

        // 1. Universal Docs
        out.push_str("## 1. Dokumentasi Universal & Kebijakan (Universal)\n\n");
        if universal_docs.is_empty() {
            out.push_str("_Belum ada dokumen universal. Tambahkan file markdown di `knowledge/universal/` atau `knowledge/facts.md`._\n\n");
        } else {
            out.push_str("| Dokumen | Judul / Deskripsi | Tautan Berkas |\n");
            out.push_str("|---|---|---|\n");
            for doc in &universal_docs {
                let summary_preview = if doc.summary.chars().count() > 70 {
                    let trunc: String = doc.summary.chars().take(67).collect();
                    format!("{}...", trunc)
                } else {
                    doc.summary.clone()
                };
                out.push_str(&format!(
                    "| **{}** | {} | [`{}`]({}) |\n",
                    doc.title, summary_preview, doc.filename, doc.path
                ));
            }
            out.push('\n');
        }

        // 2. Activities Matrix
        out.push_str("## 2. Matriks Aktivitas & Program Kerja (Activities)\n\n");
        if activities.is_empty() {
            out.push_str("_Belum ada aktivitas tercatat. Buat folder di `knowledge/kegiatan/<nama>/<periode>/README.md`._\n\n");
        } else {
            out.push_str("| Periode | Nama Aktivitas | PIC / Peran | Status | Tenggat Waktu | Berkas |\n");
            out.push_str("|---|---|---|---|---|---|\n");
            for act in &activities {
                let pic_str = act.pic.as_deref().unwrap_or("-");
                let deadline_str = act.deadline.as_deref().unwrap_or("-");
                let path_str = act.path.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| `{}` | **{}** | {} | `{}` | `{}` | [`README.md`]({}) |\n",
                    act.periode, act.nama, pic_str, act.status, deadline_str, path_str
                ));
            }
            out.push('\n');
        }

        // 3. Upcoming Deadlines
        out.push_str("## 3. Ringkasan Tenggat Waktu Mendatang (Deadlines)\n\n");
        let mut with_deadlines: Vec<_> = activities
            .iter()
            .filter(|a| a.deadline.is_some() && a.status != "selesai" && a.status != "completed" && a.status != "archived")
            .collect();

        with_deadlines.sort_by(|a, b| a.deadline.cmp(&b.deadline));

        if with_deadlines.is_empty() {
            out.push_str("_Tidak ada tenggat waktu aktif yang mendesak._\n\n");
        } else {
            for act in with_deadlines {
                let d = act.deadline.as_deref().unwrap_or("TBD");
                let pic = act.pic.as_deref().unwrap_or("Tim");
                out.push_str(&format!(
                    "- **{}**: {} (Periode: `{}`, PIC: {})\n",
                    d, act.nama, act.periode, pic
                ));
            }
            out.push('\n');
        }

        // 4. Archives
        out.push_str("## 4. Arsip Riwayat Obrolan WhatsApp (SQLite FTS5)\n\n");
        if archives.is_empty() {
            out.push_str("_Belum ada arsip obrolan yang diimpor. Gunakan `aina archive import` untuk mengimpor file export `.zip`._\n\n");
        } else {
            out.push_str("| Nama Arsip | Total Pesan | Partisipan | Rentang Tanggal | Ukuran |\n");
            out.push_str("|---|---|---|---|---|\n");
            for arc in archives {
                let earliest = arc.earliest_date.as_deref().unwrap_or("-");
                let latest = arc.latest_date.as_deref().unwrap_or("-");
                let date_range = format!("{} s.d. {}", earliest, latest);
                let size_kb = (arc.file_size_bytes as f64) / 1024.0;
                out.push_str(&format!(
                    "| **{}** | {} | {} | `{}` | {:.1} KB |\n",
                    arc.archive_name, arc.total_messages, arc.total_participants, date_range, size_kb
                ));
            }
            out.push('\n');
        }

        let index_path = knowledge_dir.join("index.md");
        std::fs::write(&index_path, &out)?;
        info!("Groomed knowledge index written to {:?}", index_path);
        Ok(out)
    }
}
