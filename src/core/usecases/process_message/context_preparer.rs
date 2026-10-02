//! Context preparation pipeline: profiling, persona engine, epistemic safeguards, and task triage.

use crate::core::domain::metacognition::{AgentCapabilityManifest, AntiConfabGate, TaskTriageEngine, TriageDecision};
use crate::core::domain::{IncomingMessage, PersonaEngine};
use crate::core::ports::{SessionStorePort, UserProfile};
use std::sync::Arc;
use tracing::{info, warn};

pub enum ContextPreparation {
    Ready {
        prompt: String,
    },
    HaltWithRejection {
        reject_message: String,
    },
}

pub struct ContextPreparer;

impl ContextPreparer {
    /// Prepares the full cognitive prompt for the agent, or yields an elegant rejection message if novel/held-out task is impossible.
    pub async fn prepare(
        msg: &IncomingMessage,
        persona_engine: &PersonaEngine,
        session_store: &Arc<dyn SessionStorePort>,
    ) -> anyhow::Result<ContextPreparation> {
        // 1. Retrieve or auto-seed sender profile with Strict OpSec guest default
        let mut profile = match session_store.get_user_profile(&msg.sender.jid).await {
            Ok(Some(p)) => p,
            Ok(None) => {
                let new_profile = UserProfile {
                    sender_jid: msg.sender.jid.clone(),
                    name: msg.sender.name.clone(),
                    role: Some("Tamu".to_string()),
                    authority_level: "guest".to_string(),
                    notes: Some("Terdaftar otomatis saat interaksi pertama (status: guest)".to_string()),
                };
                let _ = session_store.save_user_profile(&new_profile).await;
                new_profile
            }
            Err(e) => {
                warn!("Failed to fetch user profile for {}: {}", msg.sender.jid, e);
                UserProfile {
                    sender_jid: msg.sender.jid.clone(),
                    name: msg.sender.name.clone(),
                    role: Some("Tamu".to_string()),
                    authority_level: "guest".to_string(),
                    notes: None,
                }
            }
        };

        // 2. Autonomous Profiler: Dynamically extract self-introductions, callsigns, roles from incoming text
        if let Some(extracted) = crate::core::domain::AutonomousProfiler::extract_from_text(&msg.text) {
            let updated = crate::core::domain::AutonomousProfiler::merge_profile(&profile, &extracted);
            if let Err(e) = session_store.save_user_profile(&updated).await {
                warn!("Failed to update autonomous profile for {}: {}", msg.sender.jid, e);
            } else {
                profile = updated;
            }
        }

        // 3. Build prompt incorporating persona, organization context, and profiling
        let mut prompt = persona_engine.build_prompt(msg, Some(&profile));

        // 4. Smart On-Demand Episodic Memory Recall (Retrospective Intent Gate)
        let retrospective = crate::core::domain::RetrospectiveDetector::analyze(&msg.text);
        if retrospective.is_retrospective {
            let search_sender = if msg.chat_type == crate::core::domain::ChatType::DirectMessage {
                Some(msg.sender.jid.as_str())
            } else {
                None
            };

            let mut recalled_msgs = if let Some(ref query) = retrospective.extracted_query {
                session_store.search_message_history(&msg.chat_jid, search_sender, query, 5).await.unwrap_or_default()
            } else {
                Vec::new()
            };

            if recalled_msgs.is_empty() {
                recalled_msgs = session_store.get_recent_messages(&msg.chat_jid, search_sender, 3).await.unwrap_or_default();
            }

            if !recalled_msgs.is_empty() {
                prompt.push_str("\n\n---\n[RELEVANSI RIWAYAT MASA LALU (ON-DEMAND RECALL)]:\n");
                prompt.push_str("Aina mendeteksi pengguna menanyakan konteks masa lalu. Berikut riwayat relevan dari database:\n");
                for r in recalled_msgs {
                    let sender_label = if r.is_from_me {
                        "Aina"
                    } else if r.sender_jid == msg.sender.jid {
                        "Pengguna"
                    } else {
                        "Rekan Tim"
                    };
                    let location_label = if r.chat_jid.ends_with("@g.us") {
                        " [di Grup]"
                    } else {
                        ""
                    };
                    prompt.push_str(&format!("• [{}{}] {}: \"{}\"\n", r.created_at, location_label, sender_label, r.text));
                }
                prompt.push_str("(Gunakan informasi riwayat di atas secara wajar dan presisi untuk menjawab pengguna).\n");
            }
        }


        // 3. Epistemic Vigilance & Anti-Confabulation Gate
        let resolution = AntiConfabGate::evaluate_discrepancy(
            "WhatsApp Story media upload with caption field",
            &msg.text,
        );
        if let Some(guardrail) = AntiConfabGate::format_epistemic_guardrail(&resolution) {
            prompt.push_str("\n\n---\n");
            prompt.push_str(&guardrail);
        }

        // 4. Task Triage for novel/held-out tasks
        let manifest = AgentCapabilityManifest::default_manifest();
        let triage = TaskTriageEngine::triage_task(&msg.text, &manifest);
        match triage {
            TriageDecision::ElegantRejection {
                reason,
                missing_capabilities,
            } => {
                info!(
                    "Task triage rejected novel impossible task for chat {}: {}",
                    msg.chat_jid, reason
                );
                let reject_message = format!(
                    "Aina belum bisa menjalankan permintaan ini yaa 🙏\n\n*Alasan:* {}\n*Batasan Teknis:* Kapabilitas {} belum tersedia di lingkungan saat ini.",
                    reason,
                    missing_capabilities.join(", ")
                );
                Ok(ContextPreparation::HaltWithRejection { reject_message })
            }
            TriageDecision::GracefulDegradation {
                suggested_alternative,
                reason,
                ..
            } => {
                prompt.push_str(&format!(
                    "\n\n---\n[CATATAN TRIAGE KAPABILITAS]: Tugas ini melampaui kemampuan native ({}). Alihkan atau tawarkan alternatif elegan: {}.",
                    reason, suggested_alternative
                ));
                Ok(ContextPreparation::Ready { prompt })
            }
            _ => Ok(ContextPreparation::Ready { prompt }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::driven::SqliteSessionStore;
    use crate::core::domain::message::{ChatType, Platform, Sender, SessionRole};

    fn make_test_message(jid: &str, name: Option<&str>, text: &str) -> IncomingMessage {
        IncomingMessage {
            id: format!("msg_{}", rand::random::<u32>()),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: jid.to_string(),
            chat_type: ChatType::DirectMessage,
            sender: Sender {
                jid: jid.to_string(),
                name: name.map(|s| s.to_string()),
            },
            text: text.to_string(),
            timestamp: 1726000000,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: false,
            bot_lid: None,
            is_from_me: false,
            has_media: false,
            media_type: None,
            media_path: None,
        }
    }

    #[tokio::test]
    async fn test_context_preparer_auto_seeds_guest_authority() {
        let dir = std::env::temp_dir().join(format!("aina_test_guest_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let engine = PersonaEngine::new(
            "Persona test".to_string(),
            "Org test".to_string(),
            "6281234567890@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.test".to_string(),
            "628999888777@s.whatsapp.net".to_string(),
            None,
        );

        let unknown_jid = "6289912345678@s.whatsapp.net";
        let msg = make_test_message(unknown_jid, Some("Orang Baru"), "Halo apa kabar?");

        let prep = ContextPreparer::prepare(&msg, &engine, &store)
            .await
            .unwrap();

        match prep {
            ContextPreparation::Ready { prompt } => {
                assert!(prompt.contains("Tingkat Otoritas: GUEST"));
                assert!(prompt.contains("Strict OpSec"));
            }
            _ => panic!("Expected Ready"),
        }

        // Verify stored profile in SQLite is guest
        let stored = store.get_user_profile(unknown_jid).await.unwrap().unwrap();
        assert_eq!(stored.authority_level, "guest");
        assert_eq!(stored.role.as_deref(), Some("Tamu"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_context_preparer_autonomous_profiler_updates_profile() {
        let dir = std::env::temp_dir().join(format!("aina_test_autoprofile_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let engine = PersonaEngine::new(
            "Persona test".to_string(),
            "Org test".to_string(),
            "6281234567890@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.test".to_string(),
            "628999888777@s.whatsapp.net".to_string(),
            None,
        );

        let new_user_jid = "6287711223344@s.whatsapp.net";
        let msg = make_test_message(
            new_user_jid,
            None,
            "Halo Aina, perkenalkan saya Hendra Kusuma dari tim IPDS. Panggil saja Mas Hendra ya",
        );

        let prep = ContextPreparer::prepare(&msg, &engine, &store)
            .await
            .unwrap();

        match prep {
            ContextPreparation::Ready { prompt } => {
                assert!(prompt.contains("Mas Hendra"));
                assert!(prompt.contains("Ipds"));
            }
            _ => panic!("Expected Ready"),
        }

        // Verify SQLite profile was updated with extracted callsign and role
        let stored = store.get_user_profile(new_user_jid).await.unwrap().unwrap();
        assert_eq!(stored.name.as_deref(), Some("Mas Hendra"));
        assert!(stored.role.as_deref().unwrap().contains("Ipds"));
        assert!(stored.notes.as_deref().unwrap().contains("Autonomous Profiler"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_context_preparer_retrospective_recall_injects_recalled_messages() {
        let dir = std::env::temp_dir().join(format!("aina_test_recall_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let engine = PersonaEngine::new(
            "Persona test".to_string(),
            "Org test".to_string(),
            "6281234567890@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.test".to_string(),
            "628999888777@s.whatsapp.net".to_string(),
            None,
        );

        let chat_jid = "6281122334455@s.whatsapp.net";
        // Pre-populate chat history
        store.record_message(chat_jid, chat_jid, "Tolong siapkan database postgresql di port 5432", false).await.unwrap();
        store.record_message(chat_jid, "bot@s.whatsapp.net", "Siapp, postgresql 5432 sudah online", true).await.unwrap();

        // 1. Retrospective message asking about database
        let msg = make_test_message(chat_jid, Some("User"), "kemarin kita bahas soal database apa ya?");
        let prep = ContextPreparer::prepare(&msg, &engine, &store).await.unwrap();
        match prep {
            ContextPreparation::Ready { prompt } => {
                assert!(prompt.contains("[RELEVANSI RIWAYAT MASA LALU (ON-DEMAND RECALL)]"));
                assert!(prompt.contains("postgresql"));
                assert!(prompt.contains("5432"));
            }
            _ => panic!("Expected Ready"),
        }

        // 2. Pure greeting message: must NOT inject history
        let greeting_msg = make_test_message(chat_jid, Some("User"), "Ainaa");
        let greeting_prep = ContextPreparer::prepare(&greeting_msg, &engine, &store).await.unwrap();
        match greeting_prep {
            ContextPreparation::Ready { prompt } => {
                assert!(!prompt.contains("[RELEVANSI RIWAYAT MASA LALU (ON-DEMAND RECALL)]"));
            }
            _ => panic!("Expected Ready"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_context_preparer_cross_channel_group_to_dm_recall() {
        let dir = std::env::temp_dir().join(format!("aina_test_crosschannel_{}", rand::random::<u32>()));
        let db_file = dir.join("test.db");
        let store: Arc<dyn SessionStorePort> = Arc::new(SqliteSessionStore::new(&db_file).unwrap());

        let engine = PersonaEngine::new(
            "Persona test".to_string(),
            "Org test".to_string(),
            "6281234567890@s.whatsapp.net".to_string(),
            "Asia/Jakarta".to_string(),
            7,
            "id-ID".to_string(),
            "https://aina-wa.test".to_string(),
            "628999888777@s.whatsapp.net".to_string(),
            None,
        );

        let group_jid = "12036300112233@g.us";
        let alice_jid = "6281111111111@s.whatsapp.net";
        let bob_jid = "6282222222222@s.whatsapp.net";
        let charlie_jid = "6283333333333@s.whatsapp.net";

        // Alice and Bob chat in group_project
        store.record_message(group_jid, alice_jid, "Halo tim, untuk caching kita pakai apa?", false).await.unwrap();
        store.record_message(group_jid, bob_jid, "Kita pakai Redis untuk caching ya", false).await.unwrap();
        store.record_message(group_jid, "bot@s.whatsapp.net", "Siapp, Redis caching sudah aktif", true).await.unwrap();

        // 1. Alice asks Aina in private DM about the caching discussion
        let alice_dm_msg = IncomingMessage {
            id: "msg_alice_dm_1".to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: alice_jid.to_string(),
            chat_type: ChatType::DirectMessage,
            sender: Sender {
                jid: alice_jid.to_string(),
                name: Some("Alice".to_string()),
            },
            text: "Aina, tadi di grup kita bahas cache apa ya?".to_string(),
            timestamp: 1726000100,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: false,
            bot_lid: None,
            is_from_me: false,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let alice_prep = ContextPreparer::prepare(&alice_dm_msg, &engine, &store).await.unwrap();
        match alice_prep {
            ContextPreparation::Ready { prompt } => {
                // Must recall the group discussion since Alice is a member of that group
                assert!(prompt.contains("[RELEVANSI RIWAYAT MASA LALU (ON-DEMAND RECALL)]"));
                assert!(prompt.contains("[di Grup]"));
                assert!(prompt.contains("Redis"));
            }
            _ => panic!("Expected Ready for Alice DM"),
        }

        // 2. Charlie (who never joined or sent messages in group_project) asks in DM:
        let charlie_dm_msg = IncomingMessage {
            id: "msg_charlie_dm_1".to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: charlie_jid.to_string(),
            chat_type: ChatType::DirectMessage,
            sender: Sender {
                jid: charlie_jid.to_string(),
                name: Some("Charlie".to_string()),
            },
            text: "Aina, tadi di grup kita bahas cache apa ya?".to_string(),
            timestamp: 1726000200,
            quoted_message: None,
            mentioned_jids: vec![],
            is_bot_mentioned: false,
            bot_lid: None,
            is_from_me: false,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let charlie_prep = ContextPreparer::prepare(&charlie_dm_msg, &engine, &store).await.unwrap();
        match charlie_prep {
            ContextPreparation::Ready { prompt } => {
                // Charlie has no access to group_project, so Redis history must NOT leak to Charlie!
                assert!(!prompt.contains("Redis"));
                assert!(!prompt.contains("[di Grup]"));
            }
            _ => panic!("Expected Ready for Charlie DM"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

