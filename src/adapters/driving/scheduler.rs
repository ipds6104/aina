use crate::adapters::driven::agy_cli::{is_auto_canary_update_enabled, AgyUpdater};
use crate::core::domain::{PersonaEngine, SessionRole};
use crate::core::ports::WhatsAppPort;
use crate::core::usecases::ScheduledTickUseCase;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};

pub struct SchedulerRunner {
    usecase: Arc<ScheduledTickUseCase>,
    interval_duration: Duration,
}

impl SchedulerRunner {
    pub fn new(usecase: Arc<ScheduledTickUseCase>, interval_seconds: u64) -> Self {
        Self {
            usecase,
            interval_duration: Duration::from_secs(interval_seconds),
        }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            // Brief startup delay (15s) prevents duplicate scheduled-task execution during
            // zero-downtime blue/green container overlap while keeping /health instant.
            tokio::time::sleep(Duration::from_secs(15)).await;
            info!(
                "Starting background scheduler loop (tick every {:?})",
                self.interval_duration
            );
            let mut ticker = interval(self.interval_duration);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticker.tick().await;
                if let Err(e) = self.usecase.execute().await {
                    error!("Scheduler tick encountered error: {:?}", e);
                }
            }
        });
    }
}

/// Background Antigravity CLI (`agy`) version watchdog (Phase 21).
/// Checks the official release manifest every 6 hours, notifies Admin DM at most once
/// per new release, and optionally performs a SHA-512 + Canary-Verified atomic upgrade
/// when `AINA_AGY_AUTO_CANARY_UPDATE=true`.
pub fn start_agy_update_watchdog(
    binary_path: PathBuf,
    whatsapp: Arc<dyn WhatsAppPort>,
    persona_engine: Arc<PersonaEngine>,
) {
    tokio::spawn(async move {
        // Wait 2 minutes after boot so startup & initial traffic remain completely unaffected.
        tokio::time::sleep(Duration::from_secs(120)).await;
        info!(
            "Starting Antigravity CLI (`agy`) update watchdog (auto_canary={})",
            is_auto_canary_update_enabled()
        );

        let mut last_notified_version: Option<String> = None;
        let mut ticker = interval(Duration::from_secs(6 * 3600));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            let report = AgyUpdater::get_health_report_cached(&binary_path, true).await;
            if !report.update_available {
                continue;
            }
            let Some(latest_ver) = report.latest_version.clone() else {
                continue;
            };
            if last_notified_version.as_deref() == Some(latest_ver.as_str()) {
                continue;
            }

            let installed_ver = report
                .installed_version
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let admin_target = persona_engine.primary_admin_jid().map(str::to_string);

            if is_auto_canary_update_enabled() {
                info!(
                    "Watchdog detected agy CLI update {} -> {}; launching canary-verified upgrade...",
                    installed_ver, latest_ver
                );
                match AgyUpdater::perform_canary_update(&binary_path, false, false, false).await {
                    Ok(res) if res.ok && res.status == "updated" => {
                        last_notified_version = Some(latest_ver.clone());
                        if let Some(admin) = admin_target {
                            let mut msg = format!(
                                "*[Aina Watchdog] Upgrade Antigravity CLI (`agy`) Berhasil*\n\n\
                                 • Versi lama: `{}`\n\
                                 • Versi baru: `{}`\n\
                                 • Verifikasi: SHA-512 OK, Canary Flag & JSON Contract LULUS\n\
                                 • Rollback instan: `aina agy rollback`",
                                installed_ver,
                                res.new_version.as_deref().unwrap_or(&latest_ver)
                            );
                            if let Some(cl) = res.changelog_excerpt {
                                msg.push_str(&format!("\n\n*Ringkasan Perubahan:*\n{}", cl));
                            }
                            let _ = whatsapp
                                .send_text_with_session(&admin, &msg, None, SessionRole::PrimaryBot)
                                .await;
                        }
                    }
                    Ok(res) => {
                        last_notified_version = Some(latest_ver.clone());
                        warn!("Watchdog canary upgrade blocked for agy {}: {}", latest_ver, res.message);
                        if let Some(admin) = admin_target {
                            let msg = format!(
                                "*[Aina Watchdog] Upgrade Antigravity CLI (`agy`) Dibatalkan (Canary Guard)*\n\n\
                                 • Versi aktif (tetap aman): `{}`\n\
                                 • Versi target ditolak: `{}`\n\
                                 • Alasan: {}\n\n\
                                 Produksi tetap berjalan normal pada versi stabil sebelumnya.",
                                installed_ver, latest_ver, res.message
                            );
                            let _ = whatsapp
                                .send_text_with_session(&admin, &msg, None, SessionRole::PrimaryBot)
                                .await;
                        }
                    }
                    Err(e) => {
                        warn!("Watchdog canary upgrade failed with runtime error: {}", e);
                    }
                }
            } else {
                last_notified_version = Some(latest_ver.clone());
                let changelog = AgyUpdater::fetch_changelog_excerpt().await;
                if let Some(admin) = admin_target {
                    let mut msg = format!(
                        "*[Aina Watchdog] Pembaruan Antigravity CLI (`agy`) Tersedia*\n\n\
                         • Versi terpasang: `{}`\n\
                         • Versi terbaru: `{}`\n\
                         • Auto-update latar belakang dikunci demi kestabilan produksi.\n\
                         • Jalankan `aina agy update` untuk upgrade terverifikasi SHA-512 + Canary (zero-downtime).",
                        installed_ver, latest_ver
                    );
                    if let Some(cl) = changelog {
                        msg.push_str(&format!("\n\n*Ringkasan Perubahan:*\n{}", cl));
                    }
                    let _ = whatsapp
                        .send_text_with_session(&admin, &msg, None, SessionRole::PrimaryBot)
                        .await;
                }
            }
        }
    });
}
