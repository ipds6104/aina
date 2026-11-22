//! Pure domain model and deterministic policy for human-like, anti-spam in-flight
//! progress reporting, elapsed-time awareness, and smart fallback heartbeats.
//!
//! Architectural invariant (`GEMINI.md`): this module lives in `core/domain` and MUST NOT
//! perform any I/O, network calls, or database access.

use serde::{Deserialize, Serialize};

pub const DEFAULT_MIN_INITIAL_DELAY_SECS: i64 = 15;
pub const DEFAULT_DM_COOLDOWN_SECS: i64 = 90;
pub const DEFAULT_GROUP_COOLDOWN_SECS: i64 = 180;
pub const DEFAULT_DM_MAX_UPDATES: u32 = 6;
pub const DEFAULT_GROUP_MAX_UPDATES: u32 = 2;
pub const DEFAULT_MAX_MESSAGE_CHARS: usize = 450;

/// Silent fallback schedule (seconds of complete silence since turn start, last agent
/// progress, or last fallback). Capped intentionally to eliminate infinite 3-minute spam.
pub const DEFAULT_DM_FALLBACK_SILENCE_SECS: [i64; 2] = [300, 420];
pub const DEFAULT_GROUP_FALLBACK_SILENCE_SECS: [i64; 1] = [360];

/// Active or completed turn progress state bound to a turn's unforgeable `AINA_TURN_TOKEN`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnProgressSession {
    pub turn_token: String,
    pub chat_jid: String,
    pub chat_type: String,
    pub message_id: String,
    pub started_at_epoch: i64,
    pub last_progress_at_epoch: Option<i64>,
    pub last_progress_text: Option<String>,
    pub progress_count: u32,
    pub is_active: bool,
}

/// An approved progress message waiting in SQLite outbox for the daemon heartbeat loop
/// to deliver via `WhatsAppPort`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingProgressMessage {
    pub id: i64,
    pub turn_token: String,
    pub text: String,
    pub created_at_epoch: i64,
}

/// Deterministic anti-spam & safety-net policy for in-flight progress updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressPolicy {
    pub min_initial_delay_secs: i64,
    pub dm_cooldown_secs: i64,
    pub group_cooldown_secs: i64,
    pub dm_max_updates: u32,
    pub group_max_updates: u32,
    pub max_message_chars: usize,
    pub dm_fallback_silence_secs: [i64; 2],
    pub group_fallback_silence_secs: [i64; 1],
}

impl Default for ProgressPolicy {
    fn default() -> Self {
        Self {
            min_initial_delay_secs: DEFAULT_MIN_INITIAL_DELAY_SECS,
            dm_cooldown_secs: DEFAULT_DM_COOLDOWN_SECS,
            group_cooldown_secs: DEFAULT_GROUP_COOLDOWN_SECS,
            dm_max_updates: DEFAULT_DM_MAX_UPDATES,
            group_max_updates: DEFAULT_GROUP_MAX_UPDATES,
            max_message_chars: DEFAULT_MAX_MESSAGE_CHARS,
            dm_fallback_silence_secs: DEFAULT_DM_FALLBACK_SILENCE_SECS,
            group_fallback_silence_secs: DEFAULT_GROUP_FALLBACK_SILENCE_SECS,
        }
    }
}

impl ProgressPolicy {
    pub fn from_env() -> Self {
        let mut p = Self::default();
        if let Some(v) = env_i64("AINA_PROGRESS_MIN_INITIAL_SECS") {
            p.min_initial_delay_secs = v.max(0);
        }
        if let Some(v) = env_i64("AINA_PROGRESS_COOLDOWN_DM_SECS") {
            p.dm_cooldown_secs = v.max(10);
        }
        if let Some(v) = env_i64("AINA_PROGRESS_COOLDOWN_GROUP_SECS") {
            p.group_cooldown_secs = v.max(30);
        }
        if let Some(v) = env_u32("AINA_PROGRESS_MAX_DM") {
            p.dm_max_updates = v.max(1);
        }
        if let Some(v) = env_u32("AINA_PROGRESS_MAX_GROUP") {
            p.group_max_updates = v.max(1);
        }
        p
    }

    pub fn cooldown_for(&self, chat_type: &str) -> i64 {
        if chat_type.eq_ignore_ascii_case("group") {
            self.group_cooldown_secs
        } else {
            self.dm_cooldown_secs
        }
    }

    pub fn max_updates_for(&self, chat_type: &str) -> u32 {
        if chat_type.eq_ignore_ascii_case("group") {
            self.group_max_updates
        } else {
            self.dm_max_updates
        }
    }

    /// Computes remaining cooldown seconds before the next progress update is allowed.
    pub fn cooldown_remaining_secs(&self, session: &TurnProgressSession, now_epoch: i64) -> i64 {
        if !session.is_active {
            return 0;
        }
        let elapsed = (now_epoch - session.started_at_epoch).max(0);
        if session.progress_count == 0 {
            return (self.min_initial_delay_secs - elapsed).max(0);
        }
        if let Some(last_at) = session.last_progress_at_epoch {
            let since_last = (now_epoch - last_at).max(0);
            let cooldown = self.cooldown_for(&session.chat_type);
            return (cooldown - since_last).max(0);
        }
        0
    }

    /// Deterministically decides whether an agent-proposed progress message may be sent.
    pub fn evaluate_update(
        &self,
        session: &TurnProgressSession,
        raw_text: &str,
        now_epoch: i64,
    ) -> ProgressDecision {
        if !session.is_active {
            return ProgressDecision::Deny {
                code: "turn_inactive",
                reason: "Tur percakapan ini sudah selesai atau dibatalkan; pesan progres tidak lagi dapat dikirim."
                    .to_string(),
                retry_after_secs: None,
            };
        }

        let sanitized = sanitize_progress_text(raw_text);
        if sanitized.is_empty() {
            return ProgressDecision::Deny {
                code: "empty_message",
                reason: "Pesan progres tidak boleh kosong.".to_string(),
                retry_after_secs: None,
            };
        }

        let char_count = sanitized.chars().count();
        if char_count > self.max_message_chars {
            return ProgressDecision::Deny {
                code: "message_too_long",
                reason: format!(
                    "Pesan progres terlalu panjang ({} karakter, maks {}). Ringkas menjadi 1-2 kalimat santai; jawaban lengkap cukup ditulis di respons akhir agar tidak terkirim ganda.",
                    char_count, self.max_message_chars
                ),
                retry_after_secs: None,
            };
        }

        let max_updates = self.max_updates_for(&session.chat_type);
        if session.progress_count >= max_updates {
            return ProgressDecision::Deny {
                code: "max_updates_reached",
                reason: format!(
                    "Batas update progres untuk tur ini sudah tercapai ({}/{}). Lanjutkan eksekusi sampai tuntas dan sampaikan hasil lengkap di respons akhir.",
                    session.progress_count, max_updates
                ),
                retry_after_secs: None,
            };
        }

        let elapsed_secs = (now_epoch - session.started_at_epoch).max(0);
        if session.progress_count == 0 && elapsed_secs < self.min_initial_delay_secs {
            let wait = self.min_initial_delay_secs - elapsed_secs;
            return ProgressDecision::Deny {
                code: "too_early",
                reason: format!(
                    "Tugas baru berjalan {} detik. Hindari mengirim update terlalu dini agar tidak mengganggu; tunggu minimal {} detik lagi jika tugas memang panjang.",
                    elapsed_secs, wait
                ),
                retry_after_secs: Some(wait),
            };
        }

        if let Some(last_at) = session.last_progress_at_epoch {
            let cooldown = self.cooldown_for(&session.chat_type);
            let since_last = (now_epoch - last_at).max(0);
            if since_last < cooldown {
                let wait = cooldown - since_last;
                return ProgressDecision::Deny {
                    code: "cooldown_active",
                    reason: format!(
                        "Update progres baru saja dikirim {} detik lalu. Tunggu {} detik lagi sebelum mengirim kabar berikutnya agar tidak spam.",
                        since_last, wait
                    ),
                    retry_after_secs: Some(wait),
                };
            }
        }

        if let Some(ref prev) = session.last_progress_text {
            if normalize_for_dedup(prev) == normalize_for_dedup(&sanitized) {
                return ProgressDecision::Deny {
                    code: "duplicate_message",
                    reason: "Isi pesan progres sama persis dengan kabar sebelumnya. Hanya kirim update jika ada perkembangan nyata baru."
                        .to_string(),
                    retry_after_secs: None,
                };
            }
        }

        ProgressDecision::Allow {
            sanitized_text: sanitized,
            elapsed_secs,
        }
    }

    /// Evaluates whether the silent dead-man-switch fallback should send a reassuring note
    /// because the agent has been blocked (e.g. inside a long single shell command) with zero
    /// progress updates for an extended period.
    ///
    /// Guarantees:
    /// - Never claims false completion stages (e.g. "menuju tahap akhir").
    /// - Resets whenever the agent sends a real `aina progress` update.
    /// - Hard-capped (max 2 in DM, max 1 in Group) to eliminate infinite robot loops.
    pub fn evaluate_fallback_heartbeat(
        &self,
        session: &TurnProgressSession,
        last_fallback_at_epoch: Option<i64>,
        fallback_count: u32,
        now_epoch: i64,
    ) -> Option<String> {
        if !session.is_active {
            return None;
        }

        let is_group = session.chat_type.eq_ignore_ascii_case("group");
        let thresholds: &[i64] = if is_group {
            &self.group_fallback_silence_secs
        } else {
            &self.dm_fallback_silence_secs
        };

        let threshold = *thresholds.get(fallback_count as usize)?;
        let last_activity = session
            .started_at_epoch
            .max(session.last_progress_at_epoch.unwrap_or(0))
            .max(last_fallback_at_epoch.unwrap_or(0));

        if (now_epoch - last_activity) < threshold {
            return None;
        }

        let elapsed_human = format_elapsed_indonesian((now_epoch - session.started_at_epoch).max(0));
        if is_group {
            Some(format!(
                "Masih Aina proses yaa (berjalan {}), nanti langsung Aina kabari di sini begitu selesai.",
                elapsed_human
            ))
        } else if fallback_count == 0 {
            if session.last_progress_text.is_some() {
                Some(format!(
                    "Masih lanjut proses dari tahap sebelumnya yaa (berjalan {}), ditunggu sebentar...",
                    elapsed_human
                ))
            } else {
                Some(format!(
                    "Masih Aina proses di belakang layar yaa (berjalan {}), tugasnya agak panjang. Ketik *batal* kapan saja kalau mau dihentikan.",
                    elapsed_human
                ))
            }
        } else {
            Some(format!(
                "Masih terus berjalan yaa (berjalan {}), prosesnya memang cukup berat. Nanti langsung Aina kabari begitu beres, atau ketik *batal* kalau mau dihentikan.",
                elapsed_human
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgressDecision {
    Allow {
        sanitized_text: String,
        elapsed_secs: i64,
    },
    Deny {
        code: &'static str,
        reason: String,
        retry_after_secs: Option<i64>,
    },
}

/// Formats elapsed seconds into natural Indonesian duration text for WhatsApp chat.
pub fn format_elapsed_indonesian(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        return format!("{} detik", s);
    }
    if s < 3600 {
        let mins = s / 60;
        let rem = s % 60;
        if mins == 1 && rem > 0 {
            return format!("1 menit {} detik", rem);
        }
        return format!("{} menit", mins);
    }
    let hours = s / 3600;
    let mins = (s % 3600) / 60;
    if mins > 0 {
        format!("{} jam {} menit", hours, mins)
    } else {
        format!("{} jam", hours)
    }
}

/// Detects natural Indonesian progress check-in messages sent while a task is actively
/// running (e.g. `"Gimana, ketemu ngg aina?"`, `"udah sampai mana?"`, `"masih lama?"`),
/// while strictly avoiding false positives on substantive new task instructions.
pub fn is_in_flight_progress_inquiry(text: &str) -> bool {
    let clean = text.trim();
    if clean.is_empty() || clean.chars().count() > 90 {
        return false;
    }

    let lower = clean.to_lowercase();
    let normalized: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '/' { c } else { ' ' })
        .collect();
    let words: Vec<&str> = normalized.split_whitespace().collect();
    if words.is_empty() || words.len() > 12 {
        return false;
    }
    let joined = words.join(" ");

    // Never intercept substantive new tasks/requests that happen to contain "gimana" etc.
    let new_task_signals = [
        "tolong", "carikan", "buatkan", "bikinkan", "kirimkan", "sekalian", "jadwalkan",
        "tambahkan", "hapus", "ubah", "edit", "deploy", "install", "download", "upload",
        "jelaskan", "jelasin", "rangkum", "ringkas", "analisis", "hitung",
    ];
    for sig in &new_task_signals {
        if words.iter().any(|w| w == sig) {
            return false;
        }
    }

    // Exact / classic control-plane status commands
    let exact_matches = [
        "status", "/status", "progress", "progres", "cek task", "gimana", "gimana?",
        "gimana aina", "gimana nih", "jadi gimana", "apakah ada task",
    ];
    if exact_matches.iter().any(|p| joined == *p) {
        return true;
    }

    // Natural Indonesian in-flight progress check-in phrases
    let inquiry_phrases = [
        "lagi apa", "sedang apa", "cek task", "apakah ada task",
        "ketemu ngg", "ketemu ngk", "ketemu gak", "ketemu ga", "ketemu gk",
        "ketemu belum", "ketemu blm", "udah ketemu", "sudah ketemu", "udh ketemu",
        "dapet gak", "dapet ga", "dapet ngg", "dapat gak", "dapat ga",
        "udah dapet", "sudah dapat", "udh dapet", "udah dapat",
        "udah selesai", "sudah selesai", "udh selesai", "blm selesai", "belum selesai",
        "udah kelar", "sudah kelar", "udh kelar", "udah beres", "sudah beres", "udh beres",
        "sampai mana", "sampe mana", "udah sampe", "sudah sampai",
        "masih lama", "msh lama", "masih proses", "msh proses",
        "kok lama", "lama banget", "lama bgt",
        "gimana hasilnya", "gimana progres", "gimana progress", "gimana status",
        "gimana ketemu", "gimana nih", "jadi gimana",
        "udah jadi", "sudah jadi", "udh blm", "udah belum", "sudah belum",
    ];
    inquiry_phrases.iter().any(|p| joined.contains(p))
}

/// Formats a warm, human-like immediate reply when the user checks in while a task is
/// still running, quoting Aina's latest real milestone when available.
pub fn format_in_flight_status_reply(elapsed_secs: i64, last_progress_text: Option<&str>) -> String {
    let elapsed = format_elapsed_indonesian(elapsed_secs);
    match last_progress_text.map(str::trim).filter(|s| !s.is_empty()) {
        Some(last) => format!(
            "Masih jalan kok (udah sekitar {}). Kabar terakhir tadi: \"{}\". Begitu ada perkembangan atau selesai langsung Aina kabari yaa, atau ketik *batal* kalau mau dihentikan.",
            elapsed, last
        ),
        None => format!(
            "Masih Aina kerjakan kok (udah sekitar {}). Nanti langsung Aina kabari begitu ada perkembangan atau selesai yaa, atau ketik *batal* kalau mau dihentikan.",
            elapsed
        ),
    }
}

/// Generates a concise prompt note when a queued follow-up message waited significantly
/// behind a previous long-running turn (e.g. >= 45 seconds, capped at 2 hours to ignore
/// synthetic test epochs), preventing duplicate full-length answers.
pub fn format_stale_queued_message_hint(wait_secs: i64) -> Option<String> {
    if !(45..=7200).contains(&wait_secs) {
        return None;
    }
    let waited = format_elapsed_indonesian(wait_secs);
    Some(format!(
        "[CATATAN ANTREAN PESAN TERTAHAN]:\n\
         Pesan pengirim ini sebenarnya dikirim sekitar {} yang lalu saat Aina masih menyelesaikan tugas sebelumnya yang baru saja selesai.\n\
         - Jika pertanyaan/cek progres ini SUDAH terjawab oleh balasan Aina tepat di atasnya, cukup balas sangat ringkas (1 kalimat santai, misal: 'Udah beres tuh mas, barusan banget Aina kirim hasilnya di atas yaa') dan JANGAN mengulang seluruh isi penjelasan/tautan yang sama persis.\n\
         - Jika ini permintaan baru yang berbeda, kerjakan secara normal.",
        waited
    ))
}

fn sanitize_progress_text(raw: &str) -> String {
    let collapsed: String = raw
        .replace('\r', "")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    collapsed.trim().to_string()
}

fn normalize_for_dedup(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn env_i64(key: &str) -> Option<i64> {
    std::env::var(key).ok()?.trim().parse::<i64>().ok()
}

fn env_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok()?.trim().parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_session(chat_type: &str, started_at: i64) -> TurnProgressSession {
        TurnProgressSession {
            turn_token: "tok_test_123".to_string(),
            chat_jid: if chat_type == "group" {
                "120363001@g.us".to_string()
            } else {
                "628123456@s.whatsapp.net".to_string()
            },
            chat_type: chat_type.to_string(),
            message_id: "msg_1".to_string(),
            started_at_epoch: started_at,
            last_progress_at_epoch: None,
            last_progress_text: None,
            progress_count: 0,
            is_active: true,
        }
    }

    #[test]
    fn test_policy_enforces_initial_delay_cooldown_dedup_length_and_cap() {
        let policy = ProgressPolicy::default();
        let mut s = sample_session("direct", 1000);

        // Too early (<15s)
        match policy.evaluate_update(&s, "Lagi dicek yaa", 1005) {
            ProgressDecision::Deny { code, retry_after_secs, .. } => {
                assert_eq!(code, "too_early");
                assert_eq!(retry_after_secs, Some(10));
            }
            other => panic!("expected too_early, got {:?}", other),
        }

        // Allowed after initial delay
        match policy.evaluate_update(&s, "Ketemu EPUB di Libgen, mirror agak pelan lagi Aina unduh yaa.", 1025) {
            ProgressDecision::Allow { elapsed_secs, .. } => assert_eq!(elapsed_secs, 25),
            other => panic!("expected Allow, got {:?}", other),
        }

        // Simulate recorded update #1
        s.progress_count = 1;
        s.last_progress_at_epoch = Some(1025);
        s.last_progress_text = Some("Ketemu EPUB di Libgen, mirror agak pelan lagi Aina unduh yaa.".to_string());

        // Cooldown blocks update 30s later (<90s)
        match policy.evaluate_update(&s, "Update kedua", 1055) {
            ProgressDecision::Deny { code, retry_after_secs, .. } => {
                assert_eq!(code, "cooldown_active");
                assert_eq!(retry_after_secs, Some(60));
            }
            other => panic!("expected cooldown_active, got {:?}", other),
        }

        // Dedup blocks identical message even after cooldown
        match policy.evaluate_update(&s, "  ketemu epub di libgen, mirror agak pelan lagi aina unduh yaa. ", 1200) {
            ProgressDecision::Deny { code, .. } => assert_eq!(code, "duplicate_message"),
            other => panic!("expected duplicate_message, got {:?}", other),
        }

        // Length guard prevents dumping final answer into progress
        let huge = "a".repeat(500);
        match policy.evaluate_update(&s, &huge, 1200) {
            ProgressDecision::Deny { code, .. } => assert_eq!(code, "message_too_long"),
            other => panic!("expected message_too_long, got {:?}", other),
        }

        // Group cap is stricter (2 max)
        let mut grp = sample_session("group", 1000);
        grp.progress_count = 2;
        grp.last_progress_at_epoch = Some(1300);
        match policy.evaluate_update(&grp, "Update ketiga di grup", 1600) {
            ProgressDecision::Deny { code, .. } => assert_eq!(code, "max_updates_reached"),
            other => panic!("expected max_updates_reached, got {:?}", other),
        }

        // Inactive session always denied
        s.is_active = false;
        match policy.evaluate_update(&s, "Update telat", 1300) {
            ProgressDecision::Deny { code, .. } => assert_eq!(code, "turn_inactive"),
            other => panic!("expected turn_inactive, got {:?}", other),
        }
    }

    #[test]
    fn test_fallback_heartbeat_respects_agent_progress_and_stops_spamming() {
        let policy = ProgressPolicy::default();
        let mut s = sample_session("direct", 1000);

        // Before 5 min (300s): silent
        assert!(policy.evaluate_fallback_heartbeat(&s, None, 0, 1299).is_none());

        // At 5 min (300s) of total silence: first honest fallback (no fake "tahap akhir")
        let fb1 = policy.evaluate_fallback_heartbeat(&s, None, 0, 1300).unwrap();
        assert!(fb1.contains("5 menit"));
        assert!(!fb1.contains("tahap akhir"));
        assert!(fb1.contains("*batal*"));

        // Second fallback waits 7 more minutes (420s -> 1720)
        assert!(policy.evaluate_fallback_heartbeat(&s, Some(1300), 1, 1719).is_none());
        let fb2 = policy.evaluate_fallback_heartbeat(&s, Some(1300), 1, 1720).unwrap();
        assert!(fb2.contains("12 menit"));

        // After 2 fallbacks in DM: NEVER spams again
        assert!(policy.evaluate_fallback_heartbeat(&s, Some(1720), 2, 5000).is_none());

        // If agent sends a real progress update at 1200, silence timer resets!
        s.last_progress_at_epoch = Some(1200);
        s.last_progress_text = Some("Lagi unduh master EPUB dari mirror.".to_string());
        assert!(policy.evaluate_fallback_heartbeat(&s, None, 0, 1450).is_none());
        let fb_after_progress = policy.evaluate_fallback_heartbeat(&s, None, 0, 1500).unwrap();
        assert!(fb_after_progress.contains("tahap sebelumnya"));

        // Group caps at 1 fallback after 6 min (360s)
        let grp = sample_session("group", 1000);
        assert!(policy.evaluate_fallback_heartbeat(&grp, None, 0, 1359).is_none());
        assert!(policy.evaluate_fallback_heartbeat(&grp, None, 0, 1360).is_some());
        assert!(policy.evaluate_fallback_heartbeat(&grp, Some(1360), 1, 5000).is_none());
    }

    #[test]
    fn test_in_flight_progress_inquiry_catches_natural_checkins_without_false_positives() {
        // Real transcript case & natural Indonesian variants
        assert!(is_in_flight_progress_inquiry("Gimana, ketemu ngg aina?"));
        assert!(is_in_flight_progress_inquiry("udah sampai mana?"));
        assert!(is_in_flight_progress_inquiry("masih lama gak?"));
        assert!(is_in_flight_progress_inquiry("udah kelar belum?"));
        assert!(is_in_flight_progress_inquiry("status"));
        assert!(is_in_flight_progress_inquiry("lagi apa?"));

        // Substantive new requests must NOT be swallowed as status inquiries
        assert!(!is_in_flight_progress_inquiry(
            "Sekalian tolong carikan juga versi buku The Pumpkin Plan"
        ));
        assert!(!is_in_flight_progress_inquiry(
            "Gimana cara menerapkan metode Profit First untuk bisnis jasa?"
        ));
        assert!(!is_in_flight_progress_inquiry(
            "Tolong jelaskan kenapa mirror Libgen sering lambat"
        ));
    }

    #[test]
    fn test_format_elapsed_and_stale_hint() {
        assert_eq!(format_elapsed_indonesian(42), "42 detik");
        assert_eq!(format_elapsed_indonesian(60), "1 menit");
        assert_eq!(format_elapsed_indonesian(95), "1 menit 35 detik");
        assert_eq!(format_elapsed_indonesian(480), "8 menit");
        assert_eq!(format_elapsed_indonesian(3900), "1 jam 5 menit");

        assert!(format_stale_queued_message_hint(10).is_none());
        assert!(format_stale_queued_message_hint(100_000).is_none());
        let hint = format_stale_queued_message_hint(660).unwrap();
        assert!(hint.contains("11 menit"));
        assert!(hint.contains("JANGAN mengulang"));
    }
}
