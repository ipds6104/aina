//! Scheduled task execution pipeline (DirectNotification and AgentAction).

use crate::core::domain::{
    ComplexityTier, NewMetacognitivePrediction, PersonaEngine, ScheduleParser, ScheduledTask,
    ScheduledTaskType, SessionRole, TaskDomainType,
};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct TaskExecutor;

impl TaskExecutor {
    /// Executes all scheduled tasks that are currently due.
    pub async fn execute_due_tasks(
        session_store: &Arc<dyn SessionStorePort>,
        whatsapp: &Arc<dyn WhatsAppPort>,
        agent_engine: Option<&Arc<dyn AgentEnginePort>>,
        persona_engine: Option<&Arc<PersonaEngine>>,
        timezone_offset_hours: i32,
        now_epoch: i64,
    ) {
        let due_tasks = match session_store.get_due_scheduled_tasks(now_epoch).await {
            Ok(tasks) => tasks,
            Err(e) => {
                error!("Scheduler failed to query due tasks: {}", e);
                return;
            }
        };

        if !due_tasks.is_empty() {
            info!(
                "Scheduler found {} due task(s) to execute at epoch {}",
                due_tasks.len(),
                now_epoch
            );
        }

        for task in due_tasks {
            info!(
                "Executing scheduled task #{}: '{}' (Type: {:?}, Target: {})",
                task.id, task.title, task.task_type, task.target_jid
            );

            // 1. Lock/update the task state immediately to prevent duplicate burst execution
            let (next_run, is_active) = if task.schedule_type.eq_ignore_ascii_case("once") {
                (None, false)
            } else {
                let next = ScheduleParser::compute_next_run(
                    &task.schedule_type,
                    &task.schedule_expr,
                    timezone_offset_hours,
                    now_epoch + 60,
                )
                .unwrap_or(now_epoch + 86400);
                (Some(next), true)
            };

            if let Err(e) = session_store
                .update_scheduled_task_run(task.id, now_epoch, next_run, is_active)
                .await
            {
                error!(
                    "Failed to lock/update scheduled task #{} run state: {}",
                    task.id, e
                );
                continue;
            }

            // 2. Execute task payload
            let start_instant = std::time::Instant::now();
            match task.task_type {
                ScheduledTaskType::DirectNotification => {
                    Self::execute_direct_notification(
                        &task,
                        session_store,
                        whatsapp,
                        persona_engine,
                        start_instant,
                        now_epoch,
                    )
                    .await;
                }
                ScheduledTaskType::AgentAction => {
                    Self::execute_agent_action(
                        &task,
                        session_store,
                        whatsapp,
                        agent_engine,
                        persona_engine,
                        start_instant,
                        now_epoch,
                    )
                    .await;
                }
            }
        }
    }

    async fn execute_direct_notification(
        task: &ScheduledTask,
        session_store: &Arc<dyn SessionStorePort>,
        whatsapp: &Arc<dyn WhatsAppPort>,
        persona_engine: Option<&Arc<PersonaEngine>>,
        start_instant: std::time::Instant,
        now_epoch: i64,
    ) {
        if let Err(e) = whatsapp
            .send_text_with_session(&task.target_jid, &task.payload, None, SessionRole::PrimaryBot)
            .await
        {
            let duration = start_instant.elapsed().as_secs_f64();
            let err_str = e.to_string();
            error!(
                "Failed to deliver direct notification for task #{}: {}",
                task.id, e
            );
            let _ = session_store
                .update_scheduled_task_result(task.id, "failed", Some(&err_str), duration)
                .await;
            let _ = session_store
                .record_scheduled_task_run(
                    task.id,
                    &task.title,
                    &task.target_jid,
                    "failed",
                    duration,
                    Some(&err_str),
                    None,
                )
                .await;

            if let Some(admin) = persona_engine
                .map(|p| p.admin_jid())
                .filter(|j| !j.trim().is_empty())
            {
                if admin != task.target_jid {
                    let time_str = persona_engine
                        .map(|p| p.current_local_time_string())
                        .unwrap_or_else(|| format!("Epoch: {}", now_epoch));
                    let notif_report = format!(
                        "🚨 *Laporan Kegagalan Notifikasi Terjadwal Aina*\n\n\
                         • *ID Tugas*: #{}\n\
                         • *Judul*: *{}*\n\
                         • *Target Asli*: `{}`\n\
                         • *Waktu Kejadian*: {}\n\n\
                         📋 *Rincian Error:*\n\
                         ```\n{}\n```",
                        task.id,
                        task.title,
                        task.target_jid,
                        time_str,
                        err_str.trim()
                    );
                    let _ = whatsapp
                        .send_text_with_session(admin, &notif_report, None, SessionRole::PrimaryBot)
                        .await;
                }
            }
        } else {
            let duration = start_instant.elapsed().as_secs_f64();
            info!(
                "Delivered scheduled notification for task #{} to {}",
                task.id, task.target_jid
            );
            let _ = session_store
                .record_message(&task.target_jid, "bot", &task.payload, true)
                .await;
            let _ = session_store
                .update_scheduled_task_result(task.id, "success", None, duration)
                .await;
            let _ = session_store
                .record_scheduled_task_run(
                    task.id,
                    &task.title,
                    &task.target_jid,
                    "success",
                    duration,
                    None,
                    Some(&task.payload),
                )
                .await;
        }
    }

    async fn execute_agent_action(
        task: &ScheduledTask,
        session_store: &Arc<dyn SessionStorePort>,
        whatsapp: &Arc<dyn WhatsAppPort>,
        agent_engine: Option<&Arc<dyn AgentEnginePort>>,
        persona_engine: Option<&Arc<PersonaEngine>>,
        start_instant: std::time::Instant,
        now_epoch: i64,
    ) {
        let agent = match agent_engine {
            Some(a) => a,
            None => {
                warn!(
                    "Cannot execute AgentAction task #{}: AgentEngine is not configured",
                    task.id
                );
                return;
            }
        };

        let current_time_str = persona_engine
            .map(|p| p.current_local_time_string())
            .unwrap_or_else(|| format!("Epoch: {}", now_epoch));

        let is_story = task.target_jid == "status@broadcast" || task.target_jid == "status";

        let story_delivery_rule = if is_story {
            "Target adalah Status/Story WhatsApp (24 jam).\n\
            4. ATURAN STATUS STORY (ANTI STATUS GANDA & STRICT CAPTION):\n\
               - Seluruh publikasi story (gambar berserta caption terpasang) WAJIB dilakukan langsung melalui tool: 'python3 scripts/persona_status.py post' atau 'python3 skills/whatsmeow/scripts/wa_tool.py status-send-media --file <path> --caption <caption>'.\n\
               - Caption yang diposting WAJIB murni teks pesan hangat Impact Maxxing tanpa kata pengantar atau label meta seperti 'status whatsapp story:', 'caption:', 'berikut caption...'. Jika menyusun caption, buat murni atau format strict JSON {\"caption\": \"...\"}.\n\
               - Scheduler backend TIDAK AKAN mengirim teks percakapan Anda ke status@broadcast agar TIDAK TERJADI STATUS GANDA (satu gambar + satu teks terpisah)!\n"
        } else {
            "Format ramah obrolan chat.\n\
            4. ATURAN PENGIRIMAN: Untuk pesan teks biasa, DILARANG memanggil 'wa_tool.py send-text' di terminal karena teks respons Anda akan dikirim otomatis oleh scheduler backend! Namun, jika tugas ini secara spesifik meminta pengiriman berkas, dokumen, atau GAMBAR/SCREENSHOT, Anda DIPERBOLEHKAN memanggil 'python3 skills/whatsmeow/scripts/wa_tool.py send-media --to <target> --file <path_file> --caption <keterangan_singkat>'.\n"
        };

        let prompt = format!(
            "🔔 [TUGAS TERJADWAL OTOMATIS - WAKE UP CALL]\n\
            Judul Tugas: {}\n\
            Waktu Eksekusi: {}\n\
            Target Pengiriman: WhatsApp ({})\n\
            Instruksi Utama:\n{}\n\n\
            PETUNJUK FORMAT RESPON & EFISIENSI KUOTA UNTUK AINA:\n\
            1. EFISIENSI KUOTA: Lakukan maksimal 1 hingga 2 kali pencarian web (search_web) yang paling esensial. DILARANG KERAS melakukan pencarian berulang-ulang tanpa henti!\n\
            2. Susun hasil akhir secara rapi, padat, dan ramah ponsel (format WhatsApp: *tebal*, bullet points •).\n\
            3. {}\
            4. PROTOKOL NO-OP / SILENT SKIP: Jika kondisi tugas tidak terpenuhi (misalnya data tidak ada, atau hasil analisis/pengecekan kalender menunjukkan tidak perlu mengirim pesan), Anda WAJIB menjawab HANYA dengan kata '[NO_SEND]' tanpa teks lain, agar sistem tidak mengirimkan pesan yang tidak perlu ke chat WhatsApp.\n\
            5. DILARANG KERAS menyertakan laporan status teknis internal seperti 'Status: Terkirim', 'Pesan berhasil dikirim', 'Memproses pengunggahan...', dsb.\n\
            6. Berikan langsung teks hasil riset atau informasi akhir yang siap dibaca oleh penerima.",
            task.title,
            current_time_str,
            task.target_jid,
            task.payload,
            story_delivery_rule,
        );

        let pred_id = format!("sched_{}_{}", task.id, now_epoch);
        let pred = NewMetacognitivePrediction {
            prediction_id: pred_id.clone(),
            action_audit_id: None,
            task_description: format!("Scheduled Task #{}: {}", task.id, task.title),
            domain_type: if is_story {
                TaskDomainType::PersonaStatus
            } else {
                TaskDomainType::ScheduleTask
            },
            predicted_probability: if is_story { 0.88 } else { 0.92 },
            complexity_tier: if is_story {
                ComplexityTier::Medium
            } else {
                ComplexityTier::Low
            },
            identified_risks: if is_story {
                vec![
                    "Image generation timeout".to_string(),
                    "Whatsmeow gateway reachability".to_string(),
                ]
            } else {
                vec!["Search web rate limits".to_string()]
            },
            fallback_strategy: Some("Report failure to admin".to_string()),
        };
        let _ = session_store.record_metacognitive_prediction(&pred).await;

        match agent.execute(None, &prompt).await {
            Ok(res) => {
                let duration = start_instant.elapsed().as_secs_f64();
                let clean_res = res.response_text.trim();
                if !clean_res.is_empty() {
                    let is_error_output = clean_res.starts_with("⚠️")
                        || clean_res.contains("503")
                        || clean_res.contains("quota");
                    let actual_target = if is_story && is_error_output {
                        persona_engine
                            .map(|p| p.admin_jid())
                            .filter(|j| !j.trim().is_empty())
                            .unwrap_or(&task.target_jid)
                    } else {
                        &task.target_jid
                    };

                    let (status, err_msg) = if is_error_output {
                        ("failed", Some(clean_res))
                    } else {
                        ("success", None)
                    };

                    let preview = if clean_res.len() > 300 {
                        &clean_res[..300]
                    } else {
                        clean_res
                    };
                    let _ = session_store
                        .update_scheduled_task_result(task.id, status, err_msg, duration)
                        .await;
                    let _ = session_store
                        .record_scheduled_task_run(
                            task.id,
                            &task.title,
                            actual_target,
                            status,
                            duration,
                            err_msg,
                            Some(preview),
                        )
                        .await;

                    let actual_outcome = if is_error_output { 0.0 } else { 1.0 };
                    let _ = session_store
                        .resolve_metacognitive_prediction(
                            &pred_id,
                            actual_outcome,
                            duration,
                            if is_error_output {
                                Some("agent_error_or_quota")
                            } else {
                                None
                            },
                        )
                        .await;

                    if is_story {
                        info!(
                            "AgentAction task #{} for status@broadcast completed successfully. Suppressed sending conversational text to status@broadcast. Preview: {}",
                            task.id, preview
                        );
                        return;
                    }

                    // Check for Silent Skip / No-Op protocol: [NO_SEND], [SKIP], [SILENT], [SKIP_MESSAGE]
                    let is_silent_skip = clean_res.starts_with("[NO_SEND]")
                        || clean_res.starts_with("[SKIP]")
                        || clean_res.starts_with("[SILENT]")
                        || clean_res.starts_with("[SKIP_MESSAGE]")
                        || clean_res.eq_ignore_ascii_case("NO_SEND")
                        || clean_res.eq_ignore_ascii_case("SKIP");

                    if is_silent_skip {
                        info!(
                            "AgentAction task #{} requested silent skip ([NO_SEND]). Suppressed sending message to {}. Output: {}",
                            task.id, actual_target, preview
                        );
                        return;
                    }

                    if let Err(e) = whatsapp
                        .send_text_with_session(
                            actual_target,
                            clean_res,
                            None,
                            SessionRole::PrimaryBot,
                        )
                        .await
                    {
                        error!("Failed to send agent task result to WhatsApp: {}", e);
                    } else {
                        info!(
                            "Successfully executed and delivered AgentAction task #{} to {}",
                            task.id, actual_target
                        );
                        let _ = session_store
                            .record_message(actual_target, "bot", clean_res, true)
                            .await;
                    }
                } else {
                    let _ = session_store
                        .update_scheduled_task_result(task.id, "success", None, duration)
                        .await;
                    let _ = session_store
                        .record_scheduled_task_run(
                            task.id,
                            &task.title,
                            &task.target_jid,
                            "success",
                            duration,
                            None,
                            None,
                        )
                        .await;
                    let _ = session_store
                        .resolve_metacognitive_prediction(&pred_id, 1.0, duration, None)
                        .await;
                }
            }
            Err(e) => {
                let duration = start_instant.elapsed().as_secs_f64();
                let err_str = e.to_string();
                error!(
                    "Agent failed to execute scheduled task #{}: {}",
                    task.id, e
                );
                let _ = session_store
                    .update_scheduled_task_result(task.id, "failed", Some(&err_str), duration)
                    .await;
                let _ = session_store
                    .record_scheduled_task_run(
                        task.id,
                        &task.title,
                        &task.target_jid,
                        "failed",
                        duration,
                        Some(&err_str),
                        None,
                    )
                    .await;

                let _ = session_store
                    .resolve_metacognitive_prediction(
                        &pred_id,
                        0.0,
                        duration,
                        Some(&err_str),
                    )
                    .await;

                let admin_jid = persona_engine
                    .map(|p| p.admin_jid())
                    .filter(|j| !j.trim().is_empty());

                let time_str = persona_engine
                    .map(|p| p.current_local_time_string())
                    .unwrap_or_else(|| format!("Epoch: {}", now_epoch));

                let target_kind = if is_story {
                    "Status / Story WhatsApp (`status@broadcast`)".to_string()
                } else if task.target_jid.ends_with("@g.us") {
                    format!("Grup WhatsApp (`{}`)", task.target_jid)
                } else {
                    format!("Obrolan Pribadi (`{}`)", task.target_jid)
                };

                let full_err_report = format!(
                    "🚨 *Laporan Kegagalan Tugas Terjadwal Aina*\n\n\
                     • *ID Tugas*: #{}\n\
                     • *Judul*: *{}*\n\
                     • *Target Asli*: {}\n\
                     • *Waktu Kejadian*: {}\n\
                     • *Durasi Eksekusi*: {:.2} detik\n\n\
                     📋 *Rincian Lengkap Masalah / Error:*\n\
                     ```\n{}\n```\n\n\
                     💡 _Sesuai kebijakan privasi sistem, pesan error kegagalan ini tidak dikirimkan ke grup ataupun status WhatsApp, melainkan hanya dilaporkan langsung ke WhatsApp Companion/Admin._",
                    task.id,
                    task.title,
                    target_kind,
                    time_str,
                    duration,
                    err_str.trim()
                );

                if let Some(admin) = admin_jid {
                    let _ = whatsapp
                        .send_text_with_session(admin, &full_err_report, None, SessionRole::PrimaryBot)
                        .await;
                } else if !task.target_jid.ends_with("@g.us")
                    && !task.target_jid.contains("@broadcast")
                {
                    let _ = whatsapp
                        .send_text_with_session(&task.target_jid, &full_err_report, None, SessionRole::PrimaryBot)
                        .await;
                } else {
                    warn!(
                        "Suppressed failure notification to public target {} because admin_jid is not configured",
                        task.target_jid
                    );
                }
            }
        }
    }
}
