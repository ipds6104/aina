//! Background presence heartbeat and in-flight progressive reassurance timer.

use crate::core::domain::{PresenceState, SessionRole};
use crate::core::ports::WhatsAppPort;
use std::sync::Arc;

pub struct HeartbeatGuard {
    handle: Option<tokio::task::JoinHandle<()>>,
    whatsapp: Arc<dyn WhatsAppPort>,
    chat_jid: String,
    session_role: SessionRole,
}

impl HeartbeatGuard {
    pub fn new(
        handle: tokio::task::JoinHandle<()>,
        whatsapp: Arc<dyn WhatsAppPort>,
        chat_jid: String,
        session_role: SessionRole,
    ) -> Self {
        Self {
            handle: Some(handle),
            whatsapp,
            chat_jid,
            session_role,
        }
    }

    pub fn dismiss(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
            let wa = Arc::clone(&self.whatsapp);
            let jid = self.chat_jid.clone();
            let role = self.session_role;
            tokio::spawn(async move {
                let _ = wa.send_presence_with_session(&jid, PresenceState::Paused, role).await;
            });
        }
    }
}

/// Spawns a background task maintaining WhatsApp composing presence and progressive reassurance every 3 minutes.
pub fn start_presence_heartbeat(
    whatsapp: Arc<dyn WhatsAppPort>,
    chat_jid: String,
    session_role: SessionRole,
    quote_id: Option<String>,
) -> HeartbeatGuard {
    let wa_clone = Arc::clone(&whatsapp);
    let target_jid = chat_jid.clone();

    let heartbeat_handle = tokio::spawn(async move {
        let mut elapsed_secs: u64 = 0;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            elapsed_secs += 4;

            // Keep typing presence alive on WhatsApp
            let _ = wa_clone
                .send_presence_with_session(&target_jid, PresenceState::Composing, session_role)
                .await;

            // Send in-flight progressive reassurance every 3 minutes (180s, 360s, 540s...)
            if elapsed_secs > 0 && elapsed_secs % 180 == 0 {
                let minutes = elapsed_secs / 60;
                let progress_note = match minutes {
                    3 => "Masih proses Aina kerjakan yaa, ditunggu sebentar...".to_string(),
                    6 => "Masih terus Aina proses yaa, tugas ini cukup panjang tapi tetap berjalan lancar...".to_string(),
                    9 => "Masih intensif Aina proses yaa, sedang menuju tahap akhir...".to_string(),
                    _ => format!("Masih terus Aina proses yaa (berjalan {} menit), mohon ditunggu sebentar lagi...", minutes),
                };
                let _ = wa_clone
                    .send_text_with_session(
                        &target_jid,
                        &progress_note,
                        quote_id.as_deref(),
                        session_role,
                    )
                    .await;
            }
        }
    });

    HeartbeatGuard::new(heartbeat_handle, whatsapp, chat_jid, session_role)
}
