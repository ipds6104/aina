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

        // 4. Group Chat Auto Rolling Ambient Context (up to 10 preceding messages)
        if msg.chat_type == crate::core::domain::ChatType::Group {
            let limit = 11; // 10 prior messages + 1 current message (if already recorded)
            let mut recent = session_store
                .get_recent_messages(&msg.chat_jid, None, limit)
                .await
                .unwrap_or_default();

            // Exclude the current incoming message if it was already recorded in SQLite
            if let Some(last) = recent.last() {
                if !last.is_from_me && (last.sender_jid == msg.sender.jid || last.text == msg.text) {
                    recent.pop();
                }
            }

            // Keep up to 10 prior messages
            if recent.len() > 10 {
                recent = recent.split_off(recent.len() - 10);
            }

            if !recent.is_empty() {
                prompt.push_str("\n\n---\n[KONTEKS OBROLAN TERAKHIR DI GRUP (10 PESAN SEBELUMNYA)]:\n");
                prompt.push_str("Berikut alur percakapan rekan-rekan di grup sebelum pesan/tag ini. Gunakan untuk memahami konteks jika pengirim merujuk, melanjutkan, atau menimpali obrolan sebelumnya:\n");

                for r in recent {
                    let sender_display = if r.is_from_me {
                        "Aina (Bot)".to_string()
                    } else if r.sender_jid == msg.sender.jid {
                        msg.sender.name.clone().unwrap_or_else(|| {
                            r.sender_jid.split('@').next().unwrap_or(&r.sender_jid).to_string()
                        })
                    } else {
                        match session_store.get_user_profile(&r.sender_jid).await {
                            Ok(Some(p)) if p.name.as_ref().map(|n| !n.trim().is_empty()).unwrap_or(false) => {
                                let n = p.name.unwrap();
                                let num = r.sender_jid.split('@').next().unwrap_or(&r.sender_jid);
                                format!("{} ({})", n, num)
                            }
                            _ => {
                                let num = r.sender_jid.split('@').next().unwrap_or(&r.sender_jid);
                                format!("Rekan ({})", num)
                            }
                        }
                    };

                    let display_text = if r.text.chars().count() > 300 {
                        let truncated: String = r.text.chars().take(300).collect();
                        format!("{}...", truncated)
                    } else {
                        r.text
                    };

                    prompt.push_str(&format!("• [{}] {}: \"{}\"\n", r.created_at, sender_display, display_text));
                }
                prompt.push_str("(Pahami alur di atas agar tanggapan Aina nyambung, cerdas, dan luwes dengan apa yang baru saja dibahas di grup).\n");
            }
        }

        // 5. Smart On-Demand Episodic Memory Recall (Retrospective Intent Gate)
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

            if recalled_msgs.is_empty() && msg.chat_type == crate::core::domain::ChatType::DirectMessage {
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

    #[tokio::test]
    async fn test_context_preparer_group_chat_auto_rolling_context() {
        let dir = std::env::temp_dir().join(format!("aina_test_group_rolling_{}", rand::random::<u32>()));
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

        let group_jid = "12036300998877@g.us";
        let adwin_jid = "628111222333@s.whatsapp.net";
        let syihab_jid = "628222333444@s.whatsapp.net";
        let safira_jid = "628333444555@s.whatsapp.net";

        // Seed user profiles for colleagues
        let adwin_prof = UserProfile {
            sender_jid: adwin_jid.to_string(),
            name: Some("Adwin Haithay".to_string()),
            role: Some("Pegawai Senior".to_string()),
            authority_level: "staff".to_string(),
            notes: None,
        };
        store.save_user_profile(&adwin_prof).await.unwrap();

        let syihab_prof = UserProfile {
            sender_jid: syihab_jid.to_string(),
            name: Some("Syihab Alhaq".to_string()),
            role: Some("Rekan Tim".to_string()),
            authority_level: "staff".to_string(),
            notes: None,
        };
        store.save_user_profile(&syihab_prof).await.unwrap();

        // 1. Prior chat history in group
        store.record_message(group_jid, "bot@s.whatsapp.net", "PENGUMUMAN: PEMBUATAN SPK & BAST DOKTER V LEWAT AINA", true).await.unwrap();
        store.record_message(group_jid, adwin_jid, "Ini siape aina nih", false).await.unwrap();
        store.record_message(group_jid, syihab_jid, "Pegawai terbaik bulan ini bg aina", false).await.unwrap();
        store.record_message(group_jid, safira_jid, "Wkwkwk kenalan dululah aina", false).await.unwrap();

        // 2. Incoming message from Safira: "Jawab tuh aii 😂" (already recorded in store before prepare)
        let incoming_text = "Jawab tuh aii 😂";
        store.record_message(group_jid, safira_jid, incoming_text, false).await.unwrap();

        let safira_msg = IncomingMessage {
            id: "msg_safira_tag".to_string(),
            platform: Platform::WhatsApp,
            session_role: SessionRole::PrimaryBot,
            chat_jid: group_jid.to_string(),
            chat_type: ChatType::Group,
            sender: Sender {
                jid: safira_jid.to_string(),
                name: Some("Safira 57".to_string()),
            },
            text: incoming_text.to_string(),
            timestamp: 1726000300,
            quoted_message: None,
            mentioned_jids: vec!["bot@s.whatsapp.net".to_string()],
            is_bot_mentioned: true,
            bot_lid: None,
            is_from_me: false,
            has_media: false,
            media_type: None,
            media_path: None,
        };

        let prep = ContextPreparer::prepare(&safira_msg, &engine, &store).await.unwrap();
        match prep {
            ContextPreparation::Ready { prompt } => {
                // Must contain rolling ambient context header
                assert!(prompt.contains("[KONTEKS OBROLAN TERAKHIR DI GRUP (10 PESAN SEBELUMNYA)]"));
                // Must contain prior messages from colleagues with names
                assert!(prompt.contains("Adwin Haithay"));
                assert!(prompt.contains("Ini siape aina nih"));
                assert!(prompt.contains("Syihab Alhaq"));
                assert!(prompt.contains("Pegawai terbaik bulan ini bg aina"));
                assert!(prompt.contains("Aina (Bot)"));
                assert!(prompt.contains("PENGUMUMAN: PEMBUATAN SPK & BAST DOKTER V LEWAT AINA"));
                assert!(prompt.contains("Wkwkwk kenalan dululah aina"));

                // The rolling context section must NOT contain the incoming message itself as a historical message
                let rolling_section = prompt.split("[KONTEKS OBROLAN TERAKHIR DI GRUP (10 PESAN SEBELUMNYA)]")
                    .nth(1)
                    .unwrap_or("")
                    .split("(Pahami alur di atas")
                    .next()
                    .unwrap_or("");
                assert!(!rolling_section.contains(incoming_text));
            }
            _ => panic!("Expected Ready for Safira group message"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}

