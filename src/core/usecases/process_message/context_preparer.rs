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
        // 1. Retrieve or auto-seed sender profile
        let profile = match session_store.get_user_profile(&msg.sender.jid).await {
            Ok(Some(p)) => Some(p),
            Ok(None) => {
                let new_profile = UserProfile {
                    sender_jid: msg.sender.jid.clone(),
                    name: msg.sender.name.clone(),
                    role: Some("Rekan Kerja".to_string()),
                    authority_level: "staff".to_string(),
                    notes: Some("Terdaftar otomatis saat interaksi pertama".to_string()),
                };
                let _ = session_store.save_user_profile(&new_profile).await;
                Some(new_profile)
            }
            Err(e) => {
                warn!("Failed to fetch user profile for {}: {}", msg.sender.jid, e);
                None
            }
        };

        // 2. Build prompt incorporating persona, organization context, and profiling
        let mut prompt = persona_engine.build_prompt(msg, profile.as_ref());

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
