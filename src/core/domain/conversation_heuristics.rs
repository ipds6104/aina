//! Heuristic formatting for WhatsApp messages and conversational closing detection.

/// Splits a combined AI response string into multiple WhatsApp message bubbles
/// if explicit delimiter tokens are present.
pub fn split_response_into_bubbles(text: &str) -> Vec<String> {
    let delimiters = ["<<<SPLIT_CHAT>>>", "<<<NEXT_CHAT>>>", "<<<SPLIT>>>", "[SPLIT_CHAT]"];
    for delim in &delimiters {
        if text.contains(delim) {
            let parts: Vec<String> = text
                .split(delim)
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !parts.is_empty() {
                return parts;
            }
        }
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        vec![]
    } else {
        vec![trimmed.to_string()]
    }
}

/// Detects if a message is a pure conversational closing/acknowledgment/gratitude
/// that should receive a polite emoji reaction instead of an intimidating text reply.
pub fn detect_conversational_closing(text: &str) -> Option<&'static str> {
    let clean = text.trim();
    if clean.is_empty() || clean.len() > 60 {
        return None;
    }

    // Never auto-react if text has question mark
    if clean.contains('?') {
        return None;
    }

    let lower = clean.to_lowercase();

    // Check for negative or request keywords that indicate a follow-up inquiry
    let question_keywords = [
        "kenapa", "mengapa", "bagaimana", "gimana", "kapan", "siapa", "dimana", "mana",
        "tolong", "bisa tolong", "mohon bantuan", "jadwalkan", "kirimkan", "carikan",
        "tapi", "namun", "tetapi", "masih ada", "belum",
    ];
    for kw in &question_keywords {
        if lower.contains(kw) {
            return None;
        }
    }

    // Strip punctuation to normalize
    let normalized: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = normalized.split_whitespace().collect();

    if words.is_empty() || words.len() > 6 {
        return None;
    }

    let joined = words.join(" ");

    // 1. Gratitude & polite warmth -> "🙏"
    let gratitude_phrases = [
        "sama sama", "samasama", "sama2", "samik samik", "sam2",
        "terima kasih", "terimakasih", "makasih", "makasi", "makasihh", "tengkyu",
        "thank you", "thanks", "thx", "tks", "matur nuwun", "nuhun",
        "sukses selalu", "sehat selalu", "aamiin", "amin ya rabbal alamin",
        "semoga lancar", "semangat", "semangat kak",
    ];

    for pat in &gratitude_phrases {
        if joined == *pat
            || joined.starts_with(&format!("{} ", pat))
            || joined.ends_with(&format!(" {}", pat))
            || joined == format!("{} kak", pat)
            || joined == format!("{} mas", pat)
            || joined == format!("{} mba", pat)
            || joined == format!("{} pak", pat)
            || joined == format!("{} bu", pat)
            || joined == format!("{} aina", pat)
            || joined == format!("{} ya", pat)
            || joined == format!("{} yaa", pat)
            || joined == format!("{} banyak", pat)
            || joined == format!("{} infonya", pat)
        {
            return Some("🙏");
        }
    }

    // 2. Affirmation / readiness / acknowledgment -> "👍"
    let ack_phrases = [
        "siap", "siapp", "siap kak", "siap mas", "siap mba", "siap pak", "siap bu",
        "siap laksanakan", "siap makasih", "siap paham", "siap mengerti",
        "oke", "ok", "okee", "oke siap", "ok siap", "oke kak", "ok kak",
        "noted", "noted kak", "noted mas", "noted pak", "noted bu",
        "paham", "mengerti", "clear", "mantap", "mantapp", "sip", "sipp",
        "baik", "baik kak", "baik mas", "baik pak", "baik bu",
    ];

    for pat in &ack_phrases {
        if joined == *pat
            || joined.starts_with(&format!("{} ", pat))
            || joined.ends_with(&format!(" {}", pat))
            || joined == format!("{} ya", pat)
            || joined == format!("{} yaa", pat)
        {
            return Some("👍");
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_response_single_bubble() {
        let text = "Halo! Ini pesan tunggal tanpa pemisah.";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles.len(), 1);
        assert_eq!(bubbles[0], text);
    }

    #[test]
    fn test_split_response_multiple_bubbles() {
        let text = "Pesan pertama.<<<SPLIT_CHAT>>>Pesan kedua.<<<SPLIT_CHAT>>>Pesan ketiga.";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles.len(), 3);
        assert_eq!(bubbles[0], "Pesan pertama.");
        assert_eq!(bubbles[1], "Pesan kedua.");
        assert_eq!(bubbles[2], "Pesan ketiga.");
    }

    #[test]
    fn test_split_response_multiple_aliases() {
        let text = "Bagian 1<<<NEXT_CHAT>>>Bagian 2";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles.len(), 2);
        assert_eq!(bubbles[0], "Bagian 1");
        assert_eq!(bubbles[1], "Bagian 2");
    }

    #[test]
    fn test_split_response_empty_chunks_filtered() {
        let text = "<<<SPLIT>>>Pesan valid<<<SPLIT>>><<<SPLIT>>>";
        let bubbles = split_response_into_bubbles(text);
        assert_eq!(bubbles.len(), 1);
        assert_eq!(bubbles[0], "Pesan valid");
    }

    #[test]
    fn test_detect_conversational_closing_gratitude() {
        assert_eq!(detect_conversational_closing("Sama-sama kak"), Some("🙏"));
        assert_eq!(detect_conversational_closing("sama2 yaa"), Some("🙏"));
        assert_eq!(detect_conversational_closing("Terima kasih banyak!"), Some("🙏"));
        assert_eq!(detect_conversational_closing("Makasih infonya"), Some("🙏"));
        assert_eq!(detect_conversational_closing("tks"), Some("🙏"));
        assert_eq!(detect_conversational_closing("Aamiin"), Some("🙏"));
    }

    #[test]
    fn test_detect_conversational_closing_acknowledgment() {
        assert_eq!(detect_conversational_closing("Siap kak"), Some("👍"));
        assert_eq!(detect_conversational_closing("Oke siap!"), Some("👍"));
        assert_eq!(detect_conversational_closing("Noted"), Some("👍"));
        assert_eq!(detect_conversational_closing("Siap laksanakan"), Some("👍"));
        assert_eq!(detect_conversational_closing("Mantap"), Some("👍"));
        assert_eq!(detect_conversational_closing("Sipp"), Some("👍"));
    }

    #[test]
    fn test_detect_conversational_closing_ignores_inquiries_and_questions() {
        // Questions should never receive an auto-reaction
        assert_eq!(detect_conversational_closing("Kenapa SLS belum selesai?"), None);
        assert_eq!(detect_conversational_closing("Makasih kak, tapi ada kendala?"), None);
        // Follow-up requests or problem statements should not be treated as closing
        assert_eq!(detect_conversational_closing("Siap kak tapi masih ada selisih"), None);
        assert_eq!(detect_conversational_closing("Terima kasih tolong cek kembali"), None);
        // Overly long messages with narrative should proceed to normal agent
        assert_eq!(
            detect_conversational_closing("Terima kasih banyak atas infonya, nanti saya koordinasikan lagi dengan PPL desa sebelah agar cepat tuntas"),
            None
        );
    }
}
