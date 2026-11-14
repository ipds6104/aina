//! Autonomous user profiling and entity extraction engine.
//! Identifies self-introductions, preferred nicknames/callsigns, roles, and organizational context
//! from natural Indonesian conversation.

use crate::core::ports::UserProfile;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtractedProfileInfo {
    pub name: Option<String>,
    pub role: Option<String>,
    pub organization: Option<String>,
    pub callsign: Option<String>,
    pub notes: Option<String>,
}

pub struct AutonomousProfiler;

impl AutonomousProfiler {
    /// Extracts identity and profiling attributes from incoming conversational text.
    pub fn extract_from_text(text: &str) -> Option<ExtractedProfileInfo> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.contains('?') || trimmed.len() > 300 {
            return None;
        }

        let lower = trimmed.to_lowercase();
        let mut info = ExtractedProfileInfo::default();
        let mut detected = false;

        // 1. Detect Preferred Callsign: "panggil saja [X]", "panggil aku [X]", "panggil saya [X]", "panggil gw [X]"
        let callsign_triggers = [
            "panggil saja ",
            "panggil aku ",
            "panggil saya ",
            "panggil gue ",
            "panggil gw ",
            "panggil ",
        ];
        for trigger in &callsign_triggers {
            if let Some(idx) = lower.find(trigger) {
                let remainder = &trimmed[idx + trigger.len()..];
                let extracted = remainder
                    .split(&[',', '.', ';', '!', '\n', '-', '—'][..])
                    .next()
                    .unwrap_or(remainder)
                    .trim();
                let clean_name = clean_extracted_name(extracted);
                if is_valid_name(&clean_name) {
                    info.callsign = Some(clean_name.clone());
                    if info.name.is_none() {
                        info.name = Some(clean_name);
                    }
                    detected = true;
                    break;
                }
            }
        }

        // 2. Detect Name: "nama saya [X]", "namaku [X]", "nama ku [X]", "kenalkan saya [X]", "perkenalkan saya [X]"
        let name_triggers = [
            "nama saya ",
            "namaku ",
            "nama ku ",
            "nama gw ",
            "nama gue ",
            "kenalkan, saya ",
            "kenalkan saya ",
            "perkenalkan, saya ",
            "perkenalkan saya ",
            "kenalan, saya ",
            "kenalan saya ",
            "halo aina, saya ",
            "halo, saya ",
            "hai, saya ",
            "hai aina, saya ",
        ];
        for trigger in &name_triggers {
            if let Some(idx) = lower.find(trigger) {
                let remainder = &trimmed[idx + trigger.len()..];
                let extracted = remainder
                    .split(&[',', '.', ';', '!', '\n', '-', '—'][..])
                    .next()
                    .unwrap_or(remainder)
                    .trim();
                let clean_name = clean_extracted_name(extracted);
                if is_valid_name(&clean_name) {
                    if info.name.is_none() {
                        info.name = Some(clean_name);
                    }
                    detected = true;
                    break;
                }
            }
        }

        // 3. Detect Role & Department / Organization (including leadership / atasan indicators):
        // Patterns: "saya kepala [X]", "saya kabag [X]", "saya pimpinan [X]", "saya staf [X]", "tim [X]", etc.
        let role_triggers = [
            "saya kepala bagian ",
            "saya kepala seksi ",
            "saya kepala dinas ",
            "saya kepala bps ",
            "saya kepala ",
            "saya kabag ",
            "saya pimpinan ",
            "saya ketua ",
            "saya direktur ",
            "kepala bagian ",
            "kepala seksi ",
            "kepala dinas ",
            "kepala ",
            "kabag ",
            "saya staf ",
            "saya bagian ",
            "saya divisi ",
            "saya tim ",
            "saya seksi ",
            "staf bagian ",
            "staf seksi ",
            "staf divisi ",
            "staf ",
            "bagian ",
            "divisi ",
            "seksi ",
            "tim ",
            "saya dari ",
        ];
        for trigger in &role_triggers {
            if let Some(idx) = lower.find(trigger) {
                let remainder = &trimmed[idx + trigger.len()..];
                let extracted = remainder
                    .split(&[',', '.', ';', '!', '\n'][..])
                    .next()
                    .unwrap_or(remainder)
                    .trim();
                let clean_role = clean_extracted_role(extracted);
                if !clean_role.is_empty() && clean_role.len() >= 3 && clean_role.len() <= 60 {
                    let formatted_role = format_title_case(&clean_role);
                    info.role = Some(formatted_role);
                    detected = true;
                    break;
                }
            }
        }

        if detected {
            let mut notes_parts = Vec::new();
            if let Some(ref c) = info.callsign {
                notes_parts.push(format!("Preferensi panggilan: {}", c));
            }
            if let Some(ref r) = info.role {
                notes_parts.push(format!("Identifikasi peran: {}", r));
            }
            notes_parts.push("Terdeteksi otomatis via Autonomous Profiler".to_string());
            info.notes = Some(notes_parts.join(". "));
            Some(info)
        } else {
            None
        }
    }

    /// Merges freshly extracted profile cues into an existing UserProfile without downgrading authority.
    /// Strictly enforces Zero Trust Identity, Protected Name Collision Defense, and Hierarchy Claim Defense.
    pub fn merge_profile(existing: &UserProfile, extracted: &ExtractedProfileInfo) -> UserProfile {
        let mut updated = existing.clone();
        let now = chrono::Utc::now().timestamp();
        let is_verified_admin = existing.authority_level == "admin"
            || (existing.verification_status == "VERIFIED" && existing.role_hierarchy.as_deref() == Some("admin"));

        // 1. Update name & Anti-Impersonation / Protected Name Collision Defense
        if let Some(ref new_name) = extracted.name {
            updated.claimed_name = Some(new_name.clone());
            updated.claimed_at_epoch = Some(now);

            let is_protected = is_protected_name(new_name);

            if is_protected && !is_verified_admin {
                // Impersonation Attempt Detected: reject canonical name assignment
                let alert = format!(
                    "SUSPICIOUS_IDENTITY_CLAIM: Percobaan klaim nama terproteksi '{}' tanpa otorisasi.",
                    new_name
                );
                tracing::warn!(
                    "Anti-Impersonation Triggered: {} tried to claim protected name '{}'",
                    existing.sender_jid,
                    new_name
                );
                updated.verification_status = "UNCONFIRMED".to_string();
                updated.verification_notes = Some(alert);
                // Retain existing name or None to prevent name spoofing in group/context
            } else if existing.verification_status == "VERIFIED" {
                // Verified name is locked and cannot be silently altered via chat cues
                if updated.name.is_none() {
                    updated.name = Some(new_name.clone());
                }
            } else {
                // Unconfirmed user: allow conversational address while keeping status UNCONFIRMED
                if existing.name.as_deref().unwrap_or("").trim().is_empty()
                    || existing.name.as_deref() == Some("Rekan Kerja")
                    || existing.name.as_deref() == Some("Tamu")
                    || extracted.callsign.is_some()
                {
                    updated.name = Some(new_name.clone());
                }
                if updated.verification_status != "TENTATIVE" && updated.verification_status != "VERIFIED" {
                    updated.verification_status = "UNCONFIRMED".to_string();
                }
            }
        }

        // 2. Update role & Hierarchy Claim Defense
        if let Some(ref new_role) = extracted.role {
            updated.claimed_role = Some(new_role.clone());
            updated.claimed_at_epoch = Some(now);

            // Hierarchy Claim Defense:
            // Conversational claims like "saya kepala..." or "saya direktur..."
            // MUST NEVER escalate role_hierarchy to "atasan" for unverified users!
            if existing.verification_status != "VERIFIED" {
                if updated.authority_level != "admin" {
                    updated.role_hierarchy = Some("guest".to_string());
                }
                if existing.role.as_deref().unwrap_or("").trim().is_empty()
                    || existing.role.as_deref() == Some("Rekan Kerja")
                    || existing.role.as_deref() == Some("Tamu")
                    || existing.role.as_deref() == Some("Anggota Tim")
                    || existing.role.as_deref() == Some("Anggota Grup")
                {
                    updated.role = Some(new_role.clone());
                }
            } else {
                // For already verified users, retain canonical role unless unassigned
                if existing.role.as_deref().unwrap_or("").trim().is_empty() {
                    updated.role = Some(new_role.clone());
                }
            }
        }

        // 3. Append notes
        if let Some(ref extra_notes) = extracted.notes {
            match updated.notes.as_ref() {
                Some(existing_notes) if !existing_notes.trim().is_empty() => {
                    if !existing_notes.contains(extra_notes) {
                        updated.notes = Some(format!("{}; {}", existing_notes.trim(), extra_notes));
                    }
                }
                _ => {
                    updated.notes = Some(extra_notes.clone());
                }
            }
        }

        info!(
            "Autonomous Profiler merged profile for {}: name={:?}, role={:?}, authority={}, status={}",
            updated.sender_jid, updated.name, updated.role, updated.authority_level, updated.verification_status
        );

        updated
    }
}

pub fn is_protected_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    let protected_tokens = [
        "ihza",
        "karunia",
        "bg ihza",
        "bang ihza",
        "mas ihza",
        "admin",
        "administrator",
        "aina",
        "bot",
        "owner",
        "superadmin",
    ];
    for token in &protected_tokens {
        if lower == *token || lower.contains(token) {
            return true;
        }
    }
    false
}

fn is_valid_name(s: &str) -> bool {
    if s.is_empty() || s.len() < 2 || s.len() > 40 {
        return false;
    }
    // Reject common stop words or conversational fillers
    let lower = s.to_lowercase();
    let stop_words = [
        "siapa", "kamu", "dia", "mereka", "kita", "bukan", "tidak", "belum",
        "dong", "sih", "kan", "deh", "nih", "tuh", "aja", "saja", "apa",
        "gimana", "bagaimana", "tolong", "bisa", "halo", "hai", "selamat",
    ];
    if stop_words.contains(&lower.as_str()) {
        return false;
    }
    true
}

fn clean_extracted_name(s: &str) -> String {
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut clean_words = Vec::new();
    for w in words {
        let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
        let lower = trimmed.to_lowercase();
        if lower == "aja" || lower == "saja" || lower == "ya" || lower == "yaa" || lower == "deh" {
            break;
        }
        if !trimmed.is_empty() {
            clean_words.push(trimmed);
        }
        if clean_words.len() >= 4 {
            break;
        }
    }
    let joined = clean_words.join(" ");
    format_title_case(&joined)
}

fn clean_extracted_role(s: &str) -> String {
    let trimmed = s.trim().trim_matches(|c: char| !c.is_alphanumeric());
    // Strip trailing conversational particles
    let words: Vec<&str> = trimmed.split_whitespace().collect();
    let mut clean_words = Vec::new();
    for w in words {
        let lower = w.to_lowercase();
        if lower == "ya" || lower == "yaa" || lower == "salam" || lower == "kenal" {
            break;
        }
        clean_words.push(w);
        if clean_words.len() >= 6 {
            break;
        }
    }
    clean_words.join(" ")
}

fn format_title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name_and_callsign() {
        let text = "Halo Aina, perkenalkan saya Hendra Kusuma. Panggil saja Mas Hendra ya.";
        let res = AutonomousProfiler::extract_from_text(text).expect("Should detect profile");
        assert_eq!(res.callsign.as_deref(), Some("Mas Hendra"));
        assert!(res.name.is_some());
        assert_eq!(res.name.as_deref(), Some("Mas Hendra"));
    }

    #[test]
    fn test_extract_name_and_role() {
        let text = "Kenalkan saya Budi, staf IPDS BPS.";
        let res = AutonomousProfiler::extract_from_text(text).expect("Should detect profile");
        assert_eq!(res.name.as_deref(), Some("Budi"));
        assert!(res.role.as_deref().unwrap().contains("Ipds Bps"));
    }

    #[test]
    fn test_extract_callsign_only() {
        let text = "Panggil aku Rian aja";
        let res = AutonomousProfiler::extract_from_text(text).expect("Should detect callsign");
        assert_eq!(res.callsign.as_deref(), Some("Rian"));
        assert_eq!(res.name.as_deref(), Some("Rian"));
    }

    #[test]
    fn test_merge_profile_preserves_admin_authority() {
        let existing = UserProfile {
            sender_jid: "628123456789@s.whatsapp.net".to_string(),
            name: Some("Doni".to_string()),
            role: Some("Architect".to_string()),
            authority_level: "admin".to_string(),
            notes: Some("Owner".to_string()),
            ..Default::default()
        };

        let extracted = ExtractedProfileInfo {
            name: Some("Bang Doni".to_string()),
            role: None,
            organization: None,
            callsign: Some("Bang Doni".to_string()),
            notes: Some("Preferensi panggilan: Bang Doni".to_string()),
        };

        let merged = AutonomousProfiler::merge_profile(&existing, &extracted);
        assert_eq!(merged.authority_level, "admin");
        assert_eq!(merged.name.as_deref(), Some("Bang Doni"));
        assert!(merged.notes.as_deref().unwrap().contains("Owner"));
        assert!(merged.notes.as_deref().unwrap().contains("Bang Doni"));
    }

    #[test]
    fn test_merge_profile_updates_guest() {
        let existing = UserProfile {
            sender_jid: "628999111222@s.whatsapp.net".to_string(),
            name: None,
            role: Some("Tamu".to_string()),
            authority_level: "guest".to_string(),
            notes: Some("Terdaftar otomatis saat interaksi pertama (status: guest)".to_string()),
            ..Default::default()
        };

        let extracted = ExtractedProfileInfo {
            name: Some("Siti Rahma".to_string()),
            role: Some("Staf Keuangan".to_string()),
            organization: None,
            callsign: None,
            notes: Some("Terdeteksi otomatis via Autonomous Profiler".to_string()),
        };

        let merged = AutonomousProfiler::merge_profile(&existing, &extracted);
        assert_eq!(merged.authority_level, "guest"); // Preserves guest until confirmed
        assert_eq!(merged.name.as_deref(), Some("Siti Rahma"));
        assert_eq!(merged.role.as_deref(), Some("Staf Keuangan"));
    }

    #[test]
    fn test_question_not_extracted() {
        let text = "Kamu tahu siapa nama saya?";
        let res = AutonomousProfiler::extract_from_text(text);
        assert!(res.is_none());
    }

    #[test]
    fn test_anti_impersonation_rejects_protected_admin_name() {
        let stranger = UserProfile {
            sender_jid: "628999888777@s.whatsapp.net".to_string(),
            name: None,
            role: Some("Tamu".to_string()),
            authority_level: "guest".to_string(),
            verification_status: "UNCONFIRMED".to_string(),
            ..Default::default()
        };

        let extracted = ExtractedProfileInfo {
            name: Some("Ihza Karunia".to_string()),
            role: None,
            organization: None,
            callsign: None,
            notes: Some("Terdeteksi otomatis via Autonomous Profiler".to_string()),
        };

        let merged = AutonomousProfiler::merge_profile(&stranger, &extracted);
        // Canonical name must NOT be overwritten by the protected name
        assert_eq!(merged.name, None);
        // The attempt must be recorded in claimed_name
        assert_eq!(merged.claimed_name.as_deref(), Some("Ihza Karunia"));
        // Status remains UNCONFIRMED
        assert_eq!(merged.verification_status, "UNCONFIRMED");
        // Security incident note must be recorded
        assert!(merged.verification_notes.as_deref().unwrap().contains("SUSPICIOUS_IDENTITY_CLAIM"));
    }

    #[test]
    fn test_hierarchy_claim_defense_does_not_escalate_to_atasan() {
        let stranger = UserProfile {
            sender_jid: "628999888777@s.whatsapp.net".to_string(),
            name: None,
            role: Some("Tamu".to_string()),
            authority_level: "guest".to_string(),
            verification_status: "UNCONFIRMED".to_string(),
            ..Default::default()
        };

        let extracted = ExtractedProfileInfo {
            name: Some("Pak Direktur Utama".to_string()),
            role: Some("Kepala BPS Provinsi".to_string()),
            organization: None,
            callsign: None,
            notes: Some("Terdeteksi otomatis via Autonomous Profiler".to_string()),
        };

        let merged = AutonomousProfiler::merge_profile(&stranger, &extracted);
        // Role hierarchy must NOT be escalated to "atasan"
        assert_ne!(merged.role_hierarchy.as_deref(), Some("atasan"));
        assert_eq!(merged.role_hierarchy.as_deref(), Some("guest"));
        assert_eq!(merged.authority_level, "guest");
        // Status remains UNCONFIRMED
        assert_eq!(merged.verification_status, "UNCONFIRMED");
        // Claim is stored
        assert_eq!(merged.claimed_role.as_deref(), Some("Kepala BPS Provinsi"));
    }
}
