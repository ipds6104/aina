use super::message::{ChatType, IncomingMessage};
use crate::core::ports::UserProfile;

pub struct PersonaEngine {
    persona_text: String,
    organization_text: String,
    admin_jid: String,
    pub timezone: String,
    pub timezone_offset_hours: i32,
    pub locale: String,
    pub whatsmeow_url: String,
    pub bot_jid: String,
}

impl PersonaEngine {
    pub fn new(
        persona_text: String,
        organization_text: String,
        admin_jid: String,
        timezone: String,
        timezone_offset_hours: i32,
        locale: String,
        whatsmeow_url: String,
        bot_jid: String,
    ) -> Self {
        Self {
            persona_text,
            organization_text,
            admin_jid,
            timezone,
            timezone_offset_hours,
            locale,
            whatsmeow_url,
            bot_jid,
        }
    }

    pub fn current_local_time_string(&self) -> String {
        let epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        format_local_time(epoch_secs, self.timezone_offset_hours, &self.timezone)
    }

    /// Prepares the complete prompt injected into the Antigravity agent CLI.
    pub fn build_prompt(&self, msg: &IncomingMessage, profile: Option<&UserProfile>) -> String {
        let sender_name = msg
            .sender
            .name
            .as_deref()
            .or_else(|| profile.and_then(|p| p.name.as_deref()))
            .unwrap_or("Rekan Kerja");
        let sender_jid = &msg.sender.jid;

        // Determine authority level based on ADMIN_JID or stored profile
        let is_admin_jid = !self.admin_jid.trim().is_empty()
            && (sender_jid == &self.admin_jid
                || sender_jid.replace("@s.whatsapp.net", "") == self.admin_jid.replace("@s.whatsapp.net", ""));

        let (authority_level, role_title, authority_guidance) = if is_admin_jid {
            (
                "ADMIN",
                "Penanggung Jawab Sistem / Administrator Utama",
                "Memiliki wewenang penuh atas konfigurasi sistem, penugasan teknis, dan verifikasi strategis.",
            )
        } else if let Some(p) = profile {
            let auth = p.authority_level.to_uppercase();
            let role = p.role.as_deref().unwrap_or("Anggota Tim");
            match auth.as_str() {
                "ADMIN" => (
                    "ADMIN",
                    role,
                    "Memiliki wewenang penuh atas konfigurasi dan penugasan.",
                ),
                "STAFF" | "MEMBER" => (
                    "STAFF",
                    role,
                    "Rekan kerja internal. Berhak meminta bantuan coding, analisis data, script, dan reporting. Jangan berikan akses kredensial/token rahasia.",
                ),
                _ => (
                    "GUEST",
                    role,
                    "Pihak luar / tamu belum terverifikasi. Bersikap santun dan adil (al-qist). DILARANG membocorkan data internal kantor, token, kredensial, atau mengeksekusi perintah berisiko tinggi (Strict OpSec).",
                ),
            }
        } else {
            (
                "STAFF",
                "Rekan Kerja",
                "Rekan kerja. Berhak meminta bantuan analisis data, pembuatan script, dan pengerjaan tugas standar.",
            )
        };
        
        let chat_context_str = match msg.chat_type {
            ChatType::DirectMessage => "Pesan Pribadi (DM/Japri)".to_string(),
            ChatType::Group => format!("Grup Obrolan ({})", msg.chat_jid),
        };

        let platform_name = msg.platform.to_string();
        let (platform_format_guidelines, platform_ui_context) = match msg.platform {
            super::message::Platform::WhatsApp => (
                "- DIKSI & TONE: Santun, ramah, hangat rekan kerja kantor yang cekatan, to-the-point, dan low-noise (hindari basa-basi panjang).\n\
                 - PANJANG PESAN: SINGKAT & RINGKAS (2-4 paragraf pendek atau bullet points). Sangat dilarang membuat 'wall of text' yang melelahkan di layar smartphone.\n\
                 - FORMAT WA NATIVE: Gunakan *tebal* (bintang tunggal, BUKAN **ganda**), _miring_ (garis bawah tunggal), ~coret~, `inline monospace`, dan ```blok kode```.\n\
                 - ATURAN TERLARANG WA:\n\
                   * DILARANG menggunakan heading Markdown `#`, `##`, `###` (WhatsApp tidak merender heading, hanya menampilkan tanda pagar jelek). Gunakan teks *TEBAL KAPITAL* sebagai gantinya.\n\
                   * DILARANG menggunakan tabel Markdown `| a | b |` (tabel hancur dan terpotong di layar HP). Ganti tabel dengan daftar butir `• Item: Keterangan`.\n\
                   * DILARANG menggunakan link Markdown `[teks](url)` (WhatsApp tidak mendukung hyperlink). Tulis URL mentah langsung agar WhatsApp otomatis membuatnya dapat diklik.\n\
                 - PENANGANAN KODE: Jika kode pendek (<20 baris), tampilkan dalam ```kode```. Jika kode panjang (>25 baris), simpan ke file di workspace/ dan berikan cuplikan inti serta cara menjalankannya.",
                "Pengguna membaca via smartphone / WhatsApp Web."
            ),
            super::message::Platform::WebSimulator => (
                "- DIKSI & TONE: Komprehensif, profesional, terstruktur rapi layaknya dokumentasi teknis atau asisten AI web modern (ChatGPT / Claude / Gemini Web).\n\
                 - PANJANG PESAN: FLEKSIBEL & ELABORATIF. Sajikan penjelasan mendalam, komparasi tabel, langkah rinci, dan blok kode lengkap bila relevan.\n\
                 - FORMAT MARKDOWN KAYA: Gunakan GitHub Flavored Markdown lengkap (Headings #/##/###, Bullet lists, Tables, Callout alert boxes seperti `> [!NOTE]` atau `> [!TIP]`, dan syntax-highlighted code blocks).\n\
                 - TAMPILAN: Jawaban akan dirender langsung di antarmuka Web Dashboard dengan font Google Sans dan syntax highlighting.",
                "Pengguna berinteraksi via Web Browser / Dashboard Simulator."
            ),
            _ => (
                "- DIKSI & FORMAT: Sesuaikan format teks standar dan etika komunikasi yang didukung oleh platform terkait.",
                "Pengguna berinteraksi via platform pihak ketiga."
            ),
        };

        let quoted_context = match &msg.quoted_message {
            Some(q) => format!(
                "\n[Pesan yang di-quote/reply]:\nDari {}: \"{}\"\n",
                q.sender_jid, q.text
            ),
            None => String::new(),
        };

        let current_time_str = self.current_local_time_string();
        let tz_offset_sign = if self.timezone_offset_hours >= 0 { "+" } else { "" };

        format!(
            "{persona}\n\n\
            ---\n\
            [Konteks Lingkungan Kerja & Organisasi]:\n\
            {organization}\n\n\
            ---\n\
            [Waktu Sistem & Kalender Lokal Saat Ini]:\n\
            - Waktu Sekarang: {current_time_str}\n\
            - Zona Waktu: {timezone} (UTC{tz_offset_sign}{tz_offset_hours})\n\
            - Locale: {locale}\n\
            - Catatan Temporal: Gunakan waktu lokal di atas sebagai acuan akurat penanggalan, hari, dan salam waktu (pagi/siang/sore/malam).\n\n\
            ---\n\
            [Konteks Percakapan Masuk]\n\
            - Platform: {platform_name} ({platform_ui_context})\n\
            - Ruang Obrolan: {chat_context}\n\
            - Pengirim: {sender_name} ({sender_jid})\n\
            - Profil Pengirim: {role_title} (Tingkat Otoritas: {authority_level})\n\
            - Panduan Wewenang: {authority_guidance}\n\
            {quoted_context}\
            \n\
            [Pesan dari Pengirim]:\n\
            {text}\n\n\
            ---\n\
            [Integrasi WhatsApp Gateway & Akses Sistem]:\n\
            - URL Whatsmeow Gateway: {whatsmeow_url}\n\
            - JID / Akun Bot WhatsApp: {bot_jid}\n\
            - Helper Tool Resmi: `python3 .agents/skills/whatsmeow/scripts/wa_tool.py <subcommand>` (tersedia: send-text, send-media, recent, search, stats, export-backup, groups, group-info, download-media)\n\
            - PERINGATAN KERAS: Gateway WhatsApp berada di URL di atas ({whatsmeow_url}), BUKAN di http://localhost:3000. DILARANG KERAS berasumsi, mem-probing, atau melakukan curl ke http://localhost:3000.\n\n\
            ---\n\
            [Disiplin Berpikir Internal - HANYA UNTUK INTERNAL, JANGAN PERNAH DISEBUTKAN DI CHAT]:\n\
            - Verifikasi Faktual: Selalu verifikasi data teknis dan jaringan sebelum menyimpulkan. Jangan berasumsi sepihak.\n\
            - Kehati-hatian: Tahan diri dari spekulasi saat informasi belum lengkap. Akui dengan wajar jika belum tahu.\n\
            - Rujukan Primer: Konsultasi ke dokumentasi resmi atau tool jika ragu, dan minta klarifikasi sopan jika instruksi ambigu.\n\
            - Perimeter Keamanan: Bersikap ramah dan adil, namun dilarang membocorkan token, API key, atau kredensial rahasia server.\n\
            - ATURAN LARANGAN MENYEBUT ISTILAH: Seluruh prinsip di atas adalah kompas mental dan disiplin berpikir hening (silent mental discipline). DILARANG KERAS menyebutkan, mencatut, atau menceramahi istilah internal ini (seperti kata 'Tabayyun', 'Tawaqquf', 'Ahludz-Dzikri', 'OpSec', nomor surat/ayat, atau matriks otoritas) kepada pengguna di dalam teks balasan chat. Berbicaralah secara alami, ramah, dan profesional layaknya rekan kerja biasa.\n\n\
            [Panduan Format Sesuai Platform ({platform_name})]:\n\
            {platform_format_guidelines}\n\n\
            [Instruksi Respons]:\n\
            - Balaslah secara langsung sebagai Aina kepada {sender_name} dengan memperhatikan platform, waktu lokal, dan batasan wewenang pengirim di atas.\n\
            - Ingat: ramah, cekatan, solutif, basa-basi seperlunya.\n\
            - Jika permintaan pengirim kurang jelas, kurang spesifikasi/parameter, atau ambigu, tanyakan klarifikasi secara sopan dan terarah.",
            persona = self.persona_text,
            organization = self.organization_text,
            current_time_str = current_time_str,
            timezone = self.timezone,
            tz_offset_sign = tz_offset_sign,
            tz_offset_hours = self.timezone_offset_hours,
            locale = self.locale,
            whatsmeow_url = self.whatsmeow_url,
            bot_jid = self.bot_jid,
            platform_name = platform_name,
            platform_ui_context = platform_ui_context,
            chat_context = chat_context_str,
            sender_name = sender_name,
            sender_jid = sender_jid,
            role_title = role_title,
            authority_level = authority_level,
            authority_guidance = authority_guidance,
            quoted_context = quoted_context,
            text = msg.text,
            platform_format_guidelines = platform_format_guidelines
        )
    }
}

/// Formats unix epoch seconds into Indonesian formatted date and time string.
pub fn format_local_time(epoch_secs: i64, offset_hours: i32, timezone_name: &str) -> String {
    let local_secs = epoch_secs + (offset_hours as i64 * 3600);
    let days = local_secs.div_euclid(86400);
    let rem_secs = local_secs.rem_euclid(86400);

    let hh = rem_secs / 3600;
    let mm = (rem_secs % 3600) / 60;
    let ss = rem_secs % 60;

    // Day of week: 1970-01-01 was Thursday (index 0)
    let dow_idx = days.rem_euclid(7) as usize;
    let day_names = ["Kamis", "Jumat", "Sabtu", "Minggu", "Senin", "Selasa", "Rabu"];
    let day_name = day_names[dow_idx];

    let (y, m, d) = civil_from_days(days);

    let month_names = [
        "Januari", "Februari", "Maret", "April", "Mei", "Juni",
        "Juli", "Agustus", "September", "Oktober", "November", "Desember"
    ];
    let month_name = month_names[(m - 1) as usize];

    let tz_code = if offset_hours == 7 {
        "WIB"
    } else if offset_hours == 8 {
        "WITA"
    } else if offset_hours == 9 {
        "WIT"
    } else if offset_hours == 0 {
        "UTC"
    } else {
        timezone_name
    };

    format!(
        "{}, {:02} {} {} {:02}:{:02}:{:02} {}",
        day_name, d, month_name, y, hh, mm, ss, tz_code
    )
}

/// Howard Hinnant's civil date algorithm (public domain).
/// Converts days since 1970-01-01 into (year, month, day).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let final_y = if m <= 2 { y + 1 } else { y };
    (final_y, m, d)
}
