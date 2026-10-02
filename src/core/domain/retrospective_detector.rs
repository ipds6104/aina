//! Retrospective intent detector for smart on-demand context retrieval.
//! Distinguishes between immediate/forward tasks vs questions seeking past memory.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrospectiveAnalysis {
    pub is_retrospective: bool,
    pub extracted_query: Option<String>,
}

pub struct RetrospectiveDetector;

impl RetrospectiveDetector {
    /// Evaluates if the input message is seeking information from past conversations or memory.
    pub fn analyze(text: &str) -> RetrospectiveAnalysis {
        let lower = text.trim().to_lowercase();

        // 1. Immediate greetings / trivial check: definitely not retrospective
        if lower.is_empty()
            || lower == "ainaa"
            || lower == "aina"
            || lower == "halo"
            || lower == "hai"
            || lower == "pagi"
            || lower == "siang"
            || lower == "sore"
            || lower == "malam"
            || lower == "tes"
            || lower == "test"
            || lower == "ping"
            || lower == "p"
        {
            return RetrospectiveAnalysis {
                is_retrospective: false,
                extracted_query: None,
            };
        }

        // 2. Retrospective trigger keywords & phrases in Indonesian and English
        let trigger_phrases = [
            "kemarin",
            "tadi",
            "ingat",
            "sebelumnya",
            "yang tadi",
            "yang kita bahas",
            "yang pernah",
            "pernah gak",
            "pernah tidak",
            "kamu bilang",
            "kita bahas",
            "terakhir kali",
            "waktu itu",
            "semalam",
            "tempo hari",
            "last time",
            "earlier",
            "remember",
            "previously",
            "we discussed",
            "you mentioned",
            "what did we",
            "what was the",
            "siapa namaku",
            "siapa nama saya",
            "apa kataku",
        ];

        let has_trigger = trigger_phrases.iter().any(|&p| lower.contains(p));

        if !has_trigger {
            return RetrospectiveAnalysis {
                is_retrospective: false,
                extracted_query: None,
            };
        }

        // 3. Extract candidate search term by stripping common question prefixes
        let mut clean_query = lower.clone();
        let strip_prefixes = [
            "aina ingat gak",
            "aina ingat tidak",
            "aina ingat",
            "ingat gak",
            "ingat tidak",
            "apakah kamu ingat",
            "kamu ingat",
            "kemarin kita bahas apa ya soal",
            "kemarin kita bahas tentang",
            "kemarin kita bahas soal",
            "kemarin kita bahas",
            "tadi kita bahas tentang",
            "tadi kita bahas soal",
            "tadi kita bahas",
            "tadi kamu bilang",
            "kemarin kamu bilang",
            "coba ingat",
            "coba cek riwayat",
            "cari percakapan",
        ];

        for prefix in &strip_prefixes {
            if clean_query.starts_with(prefix) {
                clean_query = clean_query[prefix.len()..].trim().to_string();
                break;
            }
        }

        // Clean punctuation
        clean_query = clean_query
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .trim()
            .to_string();

        let query_term = if clean_query.len() >= 3 {
            Some(clean_query)
        } else {
            // Fallback: extract the whole text stripped of question marks
            let fallback: String = lower
                .chars()
                .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                .collect::<String>()
                .trim()
                .to_string();
            if fallback.len() >= 3 {
                Some(fallback)
            } else {
                None
            }
        };

        RetrospectiveAnalysis {
            is_retrospective: true,
            extracted_query: query_term,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greetings_and_commands_not_retrospective() {
        assert!(!RetrospectiveDetector::analyze("Ainaa").is_retrospective);
        assert!(!RetrospectiveDetector::analyze("halo").is_retrospective);
        assert!(!RetrospectiveDetector::analyze("pagi Aina").is_retrospective);
        assert!(!RetrospectiveDetector::analyze("tolong buatkan script python crawler").is_retrospective);
        assert!(!RetrospectiveDetector::analyze("/token").is_retrospective);
    }

    #[test]
    fn test_retrospective_questions_detected() {
        let res1 = RetrospectiveDetector::analyze("kemarin kita bahas soal migrasi database apa ya?");
        assert!(res1.is_retrospective);
        assert!(res1.extracted_query.is_some());

        let res2 = RetrospectiveDetector::analyze("ingat gak API key coolify yang tadi?");
        assert!(res2.is_retrospective);

        let res3 = RetrospectiveDetector::analyze("apa port yang kamu sebutkan tadi?");
        assert!(res3.is_retrospective);

        let res4 = RetrospectiveDetector::analyze("remember what we discussed last time about tailscale?");
        assert!(res4.is_retrospective);
    }
}
