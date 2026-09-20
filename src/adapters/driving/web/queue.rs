use super::state::{ActiveTaskInfo, WebhookServerState};
use crate::core::domain::IncomingMessage;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

pub fn is_kill_or_cancel_command(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    lower == "kill"
        || lower == "stop"
        || lower == "batal"
        || lower == "cancel"
        || lower == "berhenti"
        || lower == "kill task"
        || lower == "stop task"
        || lower == "batalkan"
        || lower == "hentikan"
        || lower.starts_with("/kill")
        || lower.starts_with("/cancel")
        || lower.starts_with("/stop")
        || lower.contains("kill task")
        || lower.contains("stop task")
        || lower.contains("batalkan task")
        || lower.contains("hentikan task")
}

pub fn is_status_or_inquiry_command(text: &str) -> bool {
    let lower = text.trim().to_lowercase();
    lower == "status"
        || lower == "lagi apa"
        || lower == "lagi apa?"
        || lower == "sedang apa"
        || lower == "sedang apa?"
        || lower == "apakah ada task"
        || lower == "apakah ada task?"
        || lower == "cek task"
        || lower == "progress"
        || lower == "/status"
        || lower.contains("lagi apa")
        || lower.contains("sedang apa")
}

pub async fn dispatch_message_to_queue(state: &Arc<WebhookServerState>, msg: IncomingMessage) {
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    // 1. Intercept Kill / Cancel / Stop commands IMMEDIATELY (Control Plane Bypass)
    if is_kill_or_cancel_command(&msg.text) {
        let mut active = state.active_tasks.lock().await;
        if let Some(task_info) = active.remove(&msg.chat_jid) {
            task_info.abort_handle.abort();
            info!(
                "Aborted active running task for chat {} due to user kill command: '{}'",
                msg.chat_jid, msg.text
            );

            // 1a. Immediately dismiss typing presence on WhatsApp
            let _ = state
                .usecase
                .whatsapp()
                .send_presence_with_session(&msg.chat_jid, crate::core::domain::PresenceState::Paused, msg.session_role)
                .await;

            // 1b. Mark any in-progress audits for this chat as cancelled in the database
            let filter = crate::core::domain::ActionAuditFilter {
                chat_jid: Some(msg.chat_jid.clone()),
                status: Some("in_progress".to_string()),
                limit: Some(5),
                ..Default::default()
            };
            if let Ok(in_prog_audits) = state.session_store.query_action_audits(&filter).await {
                for in_prog in in_prog_audits {
                    let dur = (now_epoch - in_prog.created_at_epoch).max(0) as f64;
                    let _ = state.session_store.update_action_audit_result(
                        in_prog.id,
                        in_prog.conversation_id.as_deref(),
                        None,
                        Some("Dibatalkan oleh pengguna (killed by user)"),
                        "cancelled",
                        Some(dur),
                        &[],
                    ).await;
                }
            }

            let reply = "Siaapp Bang Ihza, tugas yang sedang berjalan berhasil Aina hentikan paksa (killed) 👍".to_string();
            let _ = state.session_store.record_message(&msg.chat_jid, &state.bot_jid, &reply, true).await;

            // 1c. Record the cancellation action audit
            let conv_id = state.session_store.get_conversation_id(&msg.chat_jid).await.ok().flatten();
            let kill_audit = crate::core::domain::NewWhatsAppActionAudit {
                message_id: msg.id.clone(),
                chat_jid: msg.chat_jid.clone(),
                chat_type: match msg.chat_type {
                    crate::core::domain::ChatType::Group => "group".to_string(),
                    crate::core::domain::ChatType::DirectMessage => "direct".to_string(),
                },
                sender_jid: msg.sender.jid.clone(),
                sender_name: msg.sender.name.clone(),
                decision: "respond".to_string(),
                decision_reason: "User cancelled active task via control command".to_string(),
                conversation_id: conv_id,
                status: "success".to_string(),
                input_text: msg.text.clone(),
                has_media: false,
                media_path: None,
                response_text: Some(reply.clone()),
                error_message: None,
                duration_seconds: Some(0.0),
                tools_invoked: vec!["task_kill".to_string()],
                created_at_epoch: now_epoch,
                completed_at_epoch: Some(now_epoch),
            };
            let _ = state.session_store.record_action_audit(&kill_audit).await;

            let quote_id = match msg.chat_type {
                crate::core::domain::ChatType::Group => Some(msg.id.as_str()),
                crate::core::domain::ChatType::DirectMessage => None,
            };
            let _ = state.usecase.whatsapp().send_text_with_session(&msg.chat_jid, &reply, quote_id, msg.session_role).await;
            let _ = state.usecase.whatsapp().send_presence_with_session(&msg.chat_jid, crate::core::domain::PresenceState::Paused, msg.session_role).await;
            return;
        } else {
            // Even if no active task is in memory, ensure presence is paused (clears any lingering typing on client)
            let _ = state
                .usecase
                .whatsapp()
                .send_presence_with_session(&msg.chat_jid, crate::core::domain::PresenceState::Paused, msg.session_role)
                .await;

            let reply = "Saat ini tidak ada task atau proses yang sedang berjalan kokk Bang Ihza 👍 (Kondisi sistem aman dan idle)".to_string();
            let _ = state.session_store.record_message(&msg.chat_jid, &state.bot_jid, &reply, true).await;
            let quote_id = match msg.chat_type {
                crate::core::domain::ChatType::Group => Some(msg.id.as_str()),
                crate::core::domain::ChatType::DirectMessage => None,
            };
            let _ = state.usecase.whatsapp().send_text_with_session(&msg.chat_jid, &reply, quote_id, msg.session_role).await;
            let _ = state.usecase.whatsapp().send_presence_with_session(&msg.chat_jid, crate::core::domain::PresenceState::Paused, msg.session_role).await;
            return;
        }
    }

    // 2. Intercept Status / Inquiry commands if a task is actively running
    if is_status_or_inquiry_command(&msg.text) {
        let active = state.active_tasks.lock().await;
        if let Some(task_info) = active.get(&msg.chat_jid) {
            let elapsed = (now_epoch - task_info.started_at_epoch).max(0);
            let preview = if task_info.input_text.len() > 80 {
                format!("{}...", &task_info.input_text[..80])
            } else {
                task_info.input_text.clone()
            };
            let reply = format!(
                "Saat ini Aina sedang memproses tugas:\n_{}_\n\n⏱️ *Durasi berjalan:* {} detik.\n\n💡 _Ketik *'batal'* atau *'kill'* kapan pun jika ingin menghentikan tugas ini._",
                preview, elapsed
            );
            drop(active);
            let _ = state.session_store.record_message(&msg.chat_jid, &state.bot_jid, &reply, true).await;
            let quote_id = match msg.chat_type {
                crate::core::domain::ChatType::Group => Some(msg.id.as_str()),
                crate::core::domain::ChatType::DirectMessage => None,
            };
            let _ = state.usecase.whatsapp().send_text_with_session(&msg.chat_jid, &reply, quote_id, msg.session_role).await;
            return;
        }
    }

    // 3. Normal Message Execution via Queue with Active Task Registration
    let uc = Arc::clone(&state.usecase);
    let state_ref = Arc::clone(state);
    // 3500ms (3.5s) debounce window to aggregate rapid consecutive typing bursts (e.g. multi-line thoughts)
    dispatch_incoming_message_to_queue(&state.chat_queues, msg, 300, 3500, move |m| {
        let uc = Arc::clone(&uc);
        let state_worker = Arc::clone(&state_ref);
        async move {
            let chat_jid = m.chat_jid.clone();
            let session_role = m.session_role;
            let text_preview = m.text.clone();
            let started_epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let join_handle = tokio::spawn(async move { uc.execute(m).await });
            {
                let mut active = state_worker.active_tasks.lock().await;
                active.insert(
                    chat_jid.clone(),
                    ActiveTaskInfo {
                        abort_handle: join_handle.abort_handle(),
                        started_at_epoch: started_epoch,
                        input_text: text_preview,
                    },
                );
            }

            let result = join_handle.await;

            {
                let mut active = state_worker.active_tasks.lock().await;
                active.remove(&chat_jid);
            }

            // Always dismiss typing presence regardless of success, abort, or error
            let _ = state_worker
                .usecase
                .whatsapp()
                .send_presence_with_session(&chat_jid, crate::core::domain::PresenceState::Paused, session_role)
                .await;

            match result {
                Ok(inner_res) => inner_res,
                Err(e) if e.is_cancelled() => {
                    info!("Task execution for chat {} was cancelled/aborted", chat_jid);
                    Ok(())
                }
                Err(e) => {
                    error!("Task join error for chat {}: {:?}", chat_jid, e);
                    Err(e.into())
                }
            }
        }
    })
    .await;
}

pub async fn dispatch_incoming_message_to_queue<F, Fut>(
    chat_queues: &Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::mpsc::UnboundedSender<IncomingMessage>>>>,
    msg: IncomingMessage,
    idle_timeout_secs: u64,
    debounce_ms: u64,
    handler: F,
) where
    F: Fn(IncomingMessage) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let chat_jid = msg.chat_jid.clone();
    let mut queues = chat_queues.lock().await;

    let mut needs_new_worker = false;
    if let Some(tx) = queues.get(&chat_jid) {
        if tx.is_closed() {
            needs_new_worker = true;
        }
    } else {
        needs_new_worker = true;
    }

    if needs_new_worker {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<IncomingMessage>();
        let queues_ref = Arc::clone(chat_queues);
        let worker_chat_jid = chat_jid.clone();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            info!("Started sequential FIFO message queue worker for chat {}", worker_chat_jid);
            loop {
                match tokio::time::timeout(std::time::Duration::from_secs(idle_timeout_secs), rx.recv()).await {
                    Ok(Some(mut current_msg)) => {
                        // Debounce buffer for rapid consecutive messages
                        if debounce_ms > 0 && !current_msg.has_media {
                            let mut aggregated_texts = vec![current_msg.text.clone()];
                            let debounce_delay = std::time::Duration::from_millis(debounce_ms);
                            let max_wait_deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
                            let mut pending_divergent = None;

                            loop {
                                if std::time::Instant::now() >= max_wait_deadline {
                                    break;
                                }

                                match tokio::time::timeout(debounce_delay, rx.recv()).await {
                                    Ok(Some(next_msg)) => {
                                        if next_msg.sender.jid == current_msg.sender.jid && !next_msg.has_media {
                                            info!(
                                                "Debounce buffer: combining consecutive message from {} in {}",
                                                next_msg.sender.jid, worker_chat_jid
                                            );
                                            aggregated_texts.push(next_msg.text);
                                            current_msg.id = next_msg.id; // quote/reply points to latest bubble
                                        } else {
                                            pending_divergent = Some(next_msg);
                                            break;
                                        }
                                    }
                                    Ok(None) => break,
                                    Err(_) => break, // Debounce timer expired: user finished typing!
                                }
                            }

                            // Non-blockingly drain any messages already available in queue from the same sender
                            while pending_divergent.is_none() {
                                match rx.try_recv() {
                                    Ok(extra_msg) => {
                                        if extra_msg.sender.jid == current_msg.sender.jid && !extra_msg.has_media {
                                            aggregated_texts.push(extra_msg.text);
                                            current_msg.id = extra_msg.id;
                                        } else {
                                            pending_divergent = Some(extra_msg);
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }

                            current_msg.text = aggregated_texts.join("\n");

                            if let Err(e) = handler(current_msg).await {
                                error!("Error processing queued message for {}: {:?}", worker_chat_jid, e);
                            }

                            if let Some(divergent_msg) = pending_divergent {
                                if let Err(e) = handler(divergent_msg).await {
                                    error!("Error processing queued divergent message for {}: {:?}", worker_chat_jid, e);
                                }
                            }
                        } else {
                            if let Err(e) = handler(current_msg).await {
                                error!("Error processing queued message for {}: {:?}", worker_chat_jid, e);
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        // Idle timeout reached without incoming messages
                        let mut lock = queues_ref.lock().await;
                        if rx.is_empty() {
                            lock.remove(&worker_chat_jid);
                            info!("Sequential queue worker idle timeout for chat {}, cleaned up", worker_chat_jid);
                            break;
                        }
                    }
                }
            }
        });

        queues.insert(chat_jid.clone(), tx);
    }

    if let Some(tx) = queues.get(&chat_jid) {
        if let Err(e) = tx.send(msg) {
            error!("Failed to enqueue message into queue for {}: {:?}", chat_jid, e);
        }
    }
}

