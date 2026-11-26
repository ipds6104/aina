//! Scheduled task execution pipeline (DirectNotification, DirectCommand, and AgentAction).
//!
//! Designed for non-blocking concurrent dispatch so a slow or hung scheduled job
//! can never head-of-line block fast notifications or unrelated scheduled tasks.

use super::command_runner::{execute_direct_shell_command, CommandEnvConfig};
use crate::core::domain::{
    ComplexityTier, NewMetacognitivePrediction, PersonaEngine, ScheduleParser, ScheduledTask,
    ScheduledTaskType, SessionRole, TaskDomainType,
};
use crate::core::ports::{AgentEnginePort, SessionStorePort, WhatsAppPort};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::JoinSet;
use tracing::{error, info, warn};

/// Runtime dependencies and safety ceilings for executing due scheduled tasks.
#[derive(Clone)]
pub struct TaskExecutionContext {
    pub session_store: Arc<dyn SessionStorePort>,
    pub whatsapp: Arc<dyn WhatsAppPort>,
    pub agent_engine: Option<Arc<dyn AgentEnginePort>>,
    pub persona_engine: Option<Arc<PersonaEngine>>,
    pub workspace_dir: Option<PathBuf>,
    pub command_env: Option<CommandEnvConfig>,
    pub timezone_offset_hours: i32,
    pub task_timeout: Duration,
    pub command_timeout: Duration,
    pub agent_semaphore: Arc<Semaphore>,
    pub in_flight_tasks: Arc<Mutex<HashSet<i64>>>,
}

/// Safe UTF-8 prefix truncation that never slices across multi-byte char boundaries.
fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// Formats target description for private Admin failure reports.
fn describe_target_kind(target_jid: &str, is_story: bool) -> String {
    if is_story {
        "Status / Story WhatsApp (`status@broadcast`)".to_string()
    } else if target_jid.ends_with("@g.us") {
        format!("Grup WhatsApp (`{}`)", target_jid)
    } else {
        format!("Obrolan Pribadi (`{}`)", target_jid)
    }
}

/// Sends a scheduled task failure report exclusively to Admin DM (never leaking
/// internal errors to groups or `status@broadcast`).
async fn report_failure_to_admin(
    task: &ScheduledTask,
    whatsapp: &Arc<dyn WhatsAppPort>,
    persona_engine: Option<&Arc<PersonaEngine>>,
    now_epoch: i64,
    duration: f64,
    err_str: &str,
) {
    let is_story = task.target_jid == "status@broadcast" || task.target_jid == "status";
    let admin_jid = persona_engine.and_then(|p| p.primary_admin_jid());
    let time_str = persona_engine
        .map(|p| p.current_local_time_string())
        .unwrap_or_else(|| format!("Epoch: {}", now_epoch));
    let target_kind = describe_target_kind(&task.target_jid, is_story);

    let full_err_report = format!(
        "*Laporan Kegagalan Tugas Terjadwal Aina*\n\n\
         • *ID Tugas*: #{}\n\
         • *Judul*: *{}*\n\
         • *Tipe*: `{}`\n\
         • *Target Asli*: {}\n\
         • *Waktu Kejadian*: {}\n\
         • *Durasi Eksekusi*: {:.2} detik\n\n\
         *Rincian Lengkap Masalah / Error:*\n\
         ```\n{}\n```\n\n\
         _Sesuai kebijakan privasi sistem, pesan error kegagalan ini tidak dikirimkan ke grup ataupun status WhatsApp, melainkan hanya dilaporkan langsung ke WhatsApp Companion/Admin._",
        task.id,
        task.title,
        task.task_type.as_str(),
        target_kind,
        time_str,
        duration,
        err_str.trim()
    );

    if let Some(admin) = admin_jid {
        let _ = whatsapp
            .send_text_with_session(admin, &full_err_report, None, SessionRole::PrimaryBot)
            .await;
    } else if !task.target_jid.ends_with("@g.us") && !task.target_jid.contains("@broadcast") {
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

pub struct TaskExecutor;

impl TaskExecutor {
    /// Queries and dispatches all scheduled tasks that are currently due.
    ///
    /// - When `wait_for_completion` is `false` (production daemon tick), each due task is
    ///   spawned onto Tokio so the scheduler loop returns in milliseconds and never blocks.
    /// - When `wait_for_completion` is `true` (deterministic tests), due tasks still run
    ///   concurrently via a `JoinSet` and this call awaits all of them before returning.
    pub async fn execute_due_tasks(
        ctx: TaskExecutionContext,
        now_epoch: i64,
        wait_for_completion: bool,
    ) {
        let due_tasks = match ctx.session_store.get_due_scheduled_tasks(now_epoch).await {
            Ok(tasks) => tasks,
            Err(e) => {
                error!("Scheduler failed to query due tasks: {}", e);
                return;
            }
        };

        if !due_tasks.is_empty() {
            info!(
                "Scheduler found {} due task(s) to dispatch at epoch {} (wait={})",
                due_tasks.len(),
                now_epoch,
                wait_for_completion
            );
        }

        let mut join_set = JoinSet::new();

        for task in due_tasks {
            // Anti-overlap guard: never run two instances of the exact same task ID concurrently.
            {
                let mut active = ctx.in_flight_tasks.lock().await;
                if active.contains(&task.id) {
                    warn!(
                        "Skipping scheduled task #{} ('{}'): previous run is still in-flight",
                        task.id, task.title
                    );
                    continue;
                }
                active.insert(task.id);
            }

            info!(
                "Dispatching scheduled task #{}: '{}' (Type: {:?}, Target: {})",
                task.id, task.title, task.task_type, task.target_jid
            );

            // 1. Lock/advance next_run_epoch immediately to prevent duplicate burst execution
            let (next_run, is_active) = if task.schedule_type.eq_ignore_ascii_case("once") {
                (None, false)
            } else {
                let next = ScheduleParser::compute_next_run(
                    &task.schedule_type,
                    &task.schedule_expr,
                    ctx.timezone_offset_hours,
                    now_epoch + 60,
                )
                .unwrap_or(now_epoch + 86400);
                (Some(next), true)
            };

            if let Err(e) = ctx
                .session_store
                .update_scheduled_task_run(task.id, now_epoch, next_run, is_active)
                .await
            {
                error!(
                    "Failed to lock/update scheduled task #{} run state: {}",
                    task.id, e
                );
                ctx.in_flight_tasks.lock().await.remove(&task.id);
                continue;
            }

            let task_ctx = ctx.clone();
            let fut = async move {
                let task_id = task.id;
                Self::run_single_task(task, &task_ctx, now_epoch).await;
                task_ctx.in_flight_tasks.lock().await.remove(&task_id);
            };

            if wait_for_completion {
                join_set.spawn(fut);
            } else {
                tokio::spawn(fut);
            }
        }

        if wait_for_completion {
            while let Some(res) = join_set.join_next().await {
                if let Err(e) = res {
                    error!("Scheduled task panicked during concurrent execution: {:?}", e);
                }
            }
        }
    }

    async fn run_single_task(task: ScheduledTask, ctx: &TaskExecutionContext, now_epoch: i64) {
        let start_instant = std::time::Instant::now();
        match task.task_type {
            // Fast path: DirectNotification NEVER waits behind heavy agent/script semaphore permits.
            ScheduledTaskType::DirectNotification => {
                Self::execute_direct_notification(&task, ctx, start_instant, now_epoch).await;
            }
            ScheduledTaskType::DirectCommand => {
                let _permit = match ctx.agent_semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Scheduler semaphore closed for DirectCommand #{}: {}", task.id, e);
                        return;
                    }
                };
                Self::execute_direct_command(&task, ctx, start_instant, now_epoch).await;
            }
            ScheduledTaskType::AgentAction => {
                let _permit = match ctx.agent_semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Scheduler semaphore closed for AgentAction #{}: {}", task.id, e);
                        return;
                    }
                };
                Self::execute_agent_action(&task, ctx, start_instant, now_epoch).await;
            }
        }
    }

    async fn execute_direct_notification(
        task: &ScheduledTask,
        ctx: &TaskExecutionContext,
        start_instant: std::time::Instant,
        now_epoch: i64,
    ) {
        if let Err(e) = ctx
            .whatsapp
            .send_text_with_session(&task.target_jid, &task.payload, None, SessionRole::PrimaryBot)
            .await
        {
            let duration = start_instant.elapsed().as_secs_f64();
            let err_str = e.to_string();
            error!(
                "Failed to deliver direct notification for task #{}: {}",
                task.id, e
            );
            let _ = ctx
                .session_store
                .update_scheduled_task_result(task.id, "failed", Some(&err_str), duration)
                .await;
            let _ = ctx
                .session_store
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

            if let Some(admin) = ctx.persona_engine.as_ref().and_then(|p| p.primary_admin_jid()) {
                if admin != task.target_jid {
                    report_failure_to_admin(
                        task,
                        &ctx.whatsapp,
                        ctx.persona_engine.as_ref(),
                        now_epoch,
                        duration,
                        &err_str,
                    )
                    .await;
                }
            }
        } else {
            let duration = start_instant.elapsed().as_secs_f64();
            info!(
                "Delivered scheduled notification for task #{} to {}",
                task.id, task.target_jid
            );
            let _ = ctx
                .session_store
                .record_message(&task.target_jid, "bot", &task.payload, true)
                .await;
            let _ = ctx
                .session_store
                .update_scheduled_task_result(task.id, "success", None, duration)
                .await;
            let _ = ctx
                .session_store
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

    async fn execute_direct_command(
        task: &ScheduledTask,
        ctx: &TaskExecutionContext,
        start_instant: std::time::Instant,
        now_epoch: i64,
    ) {
        let sched_chat_type = if task.target_jid.ends_with("@g.us") {
            "group"
        } else {
            "direct"
        };
        let sched_cap = crate::core::domain::TurnCapability::mint(
            "system:scheduler:direct_command",
            crate::core::domain::Authority::Admin,
            task.target_jid.clone(),
            sched_chat_type,
            format!("sched_cmd_{}", task.id),
            now_epoch,
        );
        let _ = ctx.session_store.create_turn_capability(&sched_cap).await;

        match execute_direct_shell_command(
            &task.payload,
            ctx.workspace_dir.as_deref(),
            ctx.command_env.as_ref(),
            Some(&sched_cap.token),
            Some(sched_cap.issued_at),
            ctx.command_timeout,
        )
        .await
        {
            Ok(stdout_out) => {
                let duration = start_instant.elapsed().as_secs_f64();
                let preview = truncate_chars(stdout_out.trim(), 300);
                let preview_opt = if preview.is_empty() {
                    None
                } else {
                    Some(preview.as_str())
                };
                info!(
                    "Successfully executed DirectCommand task #{} ('{}') in {:.2}s",
                    task.id, task.title, duration
                );
                let _ = ctx
                    .session_store
                    .update_scheduled_task_result(task.id, "success", None, duration)
                    .await;
                let _ = ctx
                    .session_store
                    .record_scheduled_task_run(
                        task.id,
                        &task.title,
                        &task.target_jid,
                        "success",
                        duration,
                        None,
                        preview_opt,
                    )
                    .await;
            }
            Err(e) => {
                let duration = start_instant.elapsed().as_secs_f64();
                let err_str = e.to_string();
                error!(
                    "DirectCommand task #{} ('{}') failed after {:.2}s: {}",
                    task.id, task.title, duration, err_str
                );
                let _ = ctx
                    .session_store
                    .update_scheduled_task_result(task.id, "failed", Some(&err_str), duration)
                    .await;
                let _ = ctx
                    .session_store
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
                report_failure_to_admin(
                    task,
                    &ctx.whatsapp,
                    ctx.persona_engine.as_ref(),
                    now_epoch,
                    duration,
                    &err_str,
                )
                .await;
            }
        }
    }

    async fn execute_agent_action(
        task: &ScheduledTask,
        ctx: &TaskExecutionContext,
        start_instant: std::time::Instant,
        now_epoch: i64,
    ) {
        let agent = match ctx.agent_engine.as_ref() {
            Some(a) => a,
            None => {
                warn!(
                    "Cannot execute AgentAction task #{}: AgentEngine is not configured",
                    task.id
                );
                return;
            }
        };

        let current_time_str = ctx
            .persona_engine
            .as_ref()
            .map(|p| p.current_local_time_string())
            .unwrap_or_else(|| format!("Epoch: {}", now_epoch));

        let is_story = task.target_jid == "status@broadcast" || task.target_jid == "status";
        let is_internal_silent = task.target_jid == "self"
            || task.target_jid == "internal"
            || task.target_jid.is_empty()
            || task.payload.contains("[NO_SEND]");

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

        let prompt = if is_internal_silent {
            format!(
                "[TUGAS TERJADWAL OTOMATIS - WAKE UP CALL (INTERNAL BACKGROUND WORKER)]\n\
                Judul Tugas: {}\n\
                Waktu Eksekusi: {}\n\
                Tipe Tugas: Pemeliharaan & Adaptasi Internal (Silent)\n\
                Instruksi Utama:\n{}\n\n\
                PETUNJUK EKSEKUSI TUGAS INTERNAL (BACKGROUND WORKER):\n\
                1. FOKUS EKSEKUSI & MUTASI STATE: Jalankan tool / CLI yang diperlukan secara aktif untuk mengevaluasi data, menguji sistem, memperbarui jadwal, atau merefleksikan aktivitas.\n\
                2. RISET AKTUAL: Lakukan 1 kali pencarian web (search_web) jika tugas memerlukan wawasan atau fakta terkini (tahun 2026).\n\
                3. SELESAI SECARA SILENT: Setelah seluruh mutasi file, database, atau pendaftaran aktivitas selesai dijalankan dengan sukses, akhiri jawaban Anda murni dengan kata '[NO_SEND]' agar sistem backend tidak mengirimkan pesan obrolan ke chat WhatsApp.",
                task.title,
                current_time_str,
                task.payload,
            )
        } else {
            format!(
                "[TUGAS TERJADWAL OTOMATIS - WAKE UP CALL]\n\
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
            )
        };

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
        let _ = ctx.session_store.record_metacognitive_prediction(&pred).await;

        let sched_chat_type = if task.target_jid.ends_with("@g.us") {
            "group"
        } else {
            "direct"
        };
        let sched_cap = crate::core::domain::TurnCapability::mint(
            "system:scheduler",
            crate::core::domain::Authority::Admin,
            task.target_jid.clone(),
            sched_chat_type,
            format!("sched_{}", task.id),
            now_epoch,
        );
        let _ = ctx.session_store.create_turn_capability(&sched_cap).await;

        match agent
            .execute_with_capability_and_timeout(None, &prompt, Some(&sched_cap), Some(ctx.task_timeout))
            .await
        {
            Ok(res) => {
                let duration = start_instant.elapsed().as_secs_f64();
                let clean_res = res.response_text.trim();
                if !clean_res.is_empty() {
                    let is_error_output = clean_res.starts_with("⚠️")
                        || clean_res.contains("503")
                        || clean_res.contains("quota");
                    let actual_target = if is_story && is_error_output {
                        ctx.persona_engine
                            .as_ref()
                            .and_then(|p| p.primary_admin_jid())
                            .unwrap_or(&task.target_jid)
                    } else {
                        &task.target_jid
                    };

                    let (status, err_msg) = if is_error_output {
                        ("failed", Some(clean_res))
                    } else {
                        ("success", None)
                    };

                    let preview = truncate_chars(clean_res, 300);
                    let _ = ctx
                        .session_store
                        .update_scheduled_task_result(task.id, status, err_msg, duration)
                        .await;
                    let _ = ctx
                        .session_store
                        .record_scheduled_task_run(
                            task.id,
                            &task.title,
                            actual_target,
                            status,
                            duration,
                            err_msg,
                            Some(&preview),
                        )
                        .await;

                    let actual_outcome = if is_error_output { 0.0 } else { 1.0 };
                    let _ = ctx
                        .session_store
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

                    // Check for Internal Silent or Silent Skip / No-Op protocol
                    let is_silent_skip = is_internal_silent
                        || clean_res.contains("[NO_SEND]")
                        || clean_res.contains("[SKIP]")
                        || clean_res.contains("[SILENT]")
                        || clean_res.contains("[SKIP_MESSAGE]")
                        || clean_res.eq_ignore_ascii_case("NO_SEND")
                        || clean_res.eq_ignore_ascii_case("SKIP")
                        || actual_target == "self"
                        || actual_target == "internal"
                        || actual_target.is_empty();

                    if is_silent_skip {
                        info!(
                            "AgentAction task #{} is internal silent or requested silent skip ([NO_SEND]). Suppressed sending message to {}. Output: {}",
                            task.id, actual_target, preview
                        );
                        return;
                    }

                    if let Err(e) = ctx
                        .whatsapp
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
                        let _ = ctx
                            .session_store
                            .record_message(actual_target, "bot", clean_res, true)
                            .await;
                    }
                } else {
                    let _ = ctx
                        .session_store
                        .update_scheduled_task_result(task.id, "success", None, duration)
                        .await;
                    let _ = ctx
                        .session_store
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
                    let _ = ctx
                        .session_store
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
                let _ = ctx
                    .session_store
                    .update_scheduled_task_result(task.id, "failed", Some(&err_str), duration)
                    .await;
                let _ = ctx
                    .session_store
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

                let _ = ctx
                    .session_store
                    .resolve_metacognitive_prediction(
                        &pred_id,
                        0.0,
                        duration,
                        Some(&err_str),
                    )
                    .await;

                report_failure_to_admin(
                    task,
                    &ctx.whatsapp,
                    ctx.persona_engine.as_ref(),
                    now_epoch,
                    duration,
                    &err_str,
                )
                .await;
            }
        }
    }
}
