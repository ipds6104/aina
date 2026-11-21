mod adapters;
mod config;
mod core;

use adapters::driven::{AntigravityCliAdapter, SqliteSessionStore, WhatsmeowHttpAdapter};
use adapters::driving::{create_router, SchedulerRunner, WebhookServerState};
use config::AppConfig;
use core::domain::PersonaEngine;
use core::usecases::{ProcessIncomingMessageUseCase, ScheduledTickUseCase};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 0. Automatically load .env file if present
    AppConfig::load_dotenv();

    // Check if CLI subcommands are requested (e.g. `aina archive ...`, `aina kb ...`, `aina audit ...`)
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] != "server" && args[1] != "daemon" {
        return adapters::driving::CliDispatcher::run(args).await;
    }

    // Initialize structured logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aina=info,tower_http=info,axum=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Aina - Self-Hosted Agentic AI Assistant (Powered by Antigravity CLI)");

    // 1. Load configuration and persona
    let config_path = std::env::var("AINA_CONFIG").unwrap_or_else(|_| "config/config.yaml".to_string());
    let config = AppConfig::load_from_file_or_default(&config_path);

    let persona_text = config.load_persona();
    let org_text = config.load_organization();

    // Root of Trust sanity check: without ADMIN_JID nobody can delegate access,
    // and Companion-only commands (/model, /token, group trust) become unusable.
    if config.agent.admin_jid.trim().is_empty() {
        tracing::warn!(
            "ADMIN_JID is not set and no Companion JID fallback exists. No principal will \
             resolve to `admin`, so `aina policy` mutations and Companion commands will be \
             refused. Set ADMIN_JID (phone JID + LID, comma separated) or AINA_ADMIN_JID."
        );
    } else {
        tracing::info!("Root of Trust (admin identities): {}", config.agent.admin_jid);
    }
    info!(
        "Loaded persona with {} characters, organization context with {} characters",
        persona_text.len(),
        org_text.len()
    );

    // 2. Instantiate Driven Adapters (Secondary Adapters)
    let session_store = Arc::new(SqliteSessionStore::new(&config.database.path)?);
    let agent_engine = Arc::new(AntigravityCliAdapter::with_companion(
        &config.agent.binary_path,
        &config.agent.model,
        &config.agent.workspace_dir,
        config.agent.timeout_seconds,
        &config.whatsmeow.base_url,
        &config.whatsmeow.api_key,
        config.whatsmeow.companion_base_url.clone(),
        config.whatsmeow.companion_api_key.clone(),
    ));
    let presence_tracker = Arc::new(crate::core::domain::PresenceTracker::new());
    let whatsapp = Arc::new(WhatsmeowHttpAdapter::with_companion_gateway(
        &config.whatsmeow.base_url,
        &config.whatsmeow.api_key,
        &config.whatsmeow.send_endpoint,
        &config.whatsmeow.presence_endpoint,
        config.whatsmeow.bot_session_id.clone(),
        config.whatsmeow.companion_session_id.clone(),
        config.whatsmeow.companion_base_url.clone(),
        config.whatsmeow.companion_api_key.clone(),
    ).with_presence_tracker(Arc::clone(&presence_tracker)));

    let persona_path = std::path::PathBuf::from(&config.agent.persona_file);
    let org_path = std::path::PathBuf::from(&config.agent.organization_file);

    let persona_engine = Arc::new(PersonaEngine::with_file_paths(
        persona_text,
        org_text,
        Some(persona_path),
        Some(org_path),
        config.agent.admin_jid.clone(),
        config.app.timezone.clone(),
        config.app.timezone_offset_hours,
        config.app.locale.clone(),
        config.whatsmeow.base_url.clone(),
        config.whatsmeow.bot_jid.clone(),
        Some(config.agent.workspace_dir.clone()),
    ));

    // 3. Instantiate Use Cases (Core Application Logic)
    let process_message_usecase = Arc::new(ProcessIncomingMessageUseCase::new(
        Arc::clone(&session_store) as _,
        Arc::clone(&agent_engine) as _,
        Arc::clone(&whatsapp) as _,
        Arc::clone(&persona_engine),
        config.whatsmeow.bot_jid.clone(),
        config.whatsmeow.bot_name.clone(),
        config.whatsmeow.bot_lid.clone(),
    ));

    let scheduled_tick_usecase = Arc::new(ScheduledTickUseCase::new(
        Arc::clone(&session_store) as _,
        Arc::clone(&whatsapp) as _,
        Some(Arc::clone(&agent_engine) as _),
        Some(Arc::clone(&persona_engine)),
        Some(std::path::PathBuf::from(&config.agent.workspace_dir)),
        config.app.timezone_offset_hours,
    ));

    // 3.5 Auto-Reconcile Baseline & Custom Activities
    let baseline_defs = crate::core::domain::load_default_baseline();
    let custom_dir = if std::path::Path::new("/app/data/activities").exists() {
        std::path::PathBuf::from("/app/data/activities")
    } else {
        std::path::PathBuf::from("data/activities")
    };
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    match crate::core::domain::ActivityReconciler::sync_all(
        session_store.as_ref(),
        &baseline_defs,
        Some(&custom_dir),
        config.app.timezone_offset_hours,
        now_epoch,
    ).await {
        Ok(report) => {
            tracing::info!(
                "Activity Reconciler synced: {} new baseline, {} existing baseline, {} new custom, {} updated custom",
                report.created_baseline,
                report.existing_baseline,
                report.created_custom,
                report.updated_custom
            );
        }
        Err(e) => {
            tracing::warn!("Failed during initial activity reconciliation: {}", e);
        }
    }

    // 4. Start Scheduler if enabled (Driving Adapter)
    if config.scheduler.enabled {
        let runner = SchedulerRunner::new(
            Arc::clone(&scheduled_tick_usecase),
            config.scheduler.interval_seconds,
        );
        runner.start();
    }

    // 5. Setup Code / Admin Key for First-Time Setup Security (128-bit CSPRNG fallback; never empty)
    let setup_code = std::env::var("ADMIN_KEY")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("AINA_ADMIN_KEY")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| {
            format!(
                "AINA-{}",
                adapters::driving::web::auth::generate_csprng_hex(16)
            )
        });

    let ephemeral_session_secret = adapters::driving::web::auth::generate_csprng_hex(32);

    let webhook_secret = std::env::var("AINA_WEBHOOK_SECRET")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("WHATSMEOW_WEBHOOK_SECRET")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

    let events_allow_query_key = std::env::var("AINA_EVENTS_ALLOW_QUERY_KEY")
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    let trust_proxy_headers = std::env::var("AINA_TRUST_PROXY_HEADERS")
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false);

    info!("========================================================================");
    info!("🔐 SETUP / ADMIN CODE: {}", setup_code);
    info!("Use this code in the Web Setup Wizard (/setup) to claim or update Aina.");
    if config.oidc.enabled {
        info!("🔑 SSO / OIDC ENABLED via provider: {} (Issuer: {:?})", config.oidc.provider_name, config.oidc.issuer_url);
    } else {
        info!("ℹ️ SSO / OIDC is disabled (set OIDC_ENABLED=true to activate)");
    }
    if webhook_secret.is_some() {
        info!("🛡️ Webhook auth enabled via AINA_WEBHOOK_SECRET (fail-closed)");
    } else {
        tracing::warn!(
            "AINA_WEBHOOK_SECRET is not set; POST /webhook relies on isolated internal network trust. Set AINA_WEBHOOK_SECRET if exposing /webhook publicly."
        );
    }
    info!("========================================================================");

    // 6. Setup Webhook Router & HTTP Server (Driving Adapter)
    let state = Arc::new(WebhookServerState {
        usecase: Arc::clone(&process_message_usecase),
        agent_engine: Arc::clone(&agent_engine) as _,
        session_store: Arc::clone(&session_store) as _,
        persona_engine: Arc::clone(&persona_engine),
        bot_name: config.whatsmeow.bot_name.clone(),
        bot_jid: config.whatsmeow.bot_jid.clone(),
        bot_lid: config.whatsmeow.bot_lid.clone(),
        companion_jid: config.whatsmeow.companion_jid.clone(),
        companion_name: config.whatsmeow.companion_name.clone(),
        companion_session_id: config.whatsmeow.companion_session_id.clone(),
        model: config.agent.model.clone(),
        whatsmeow_url: config.whatsmeow.base_url.clone(),
        whatsmeow_api_key: config.whatsmeow.api_key.clone(),
        companion_base_url: config.whatsmeow.companion_base_url.clone(),
        companion_api_key: config.whatsmeow.companion_api_key.clone(),
        setup_code,
        events_api_key: std::env::var("AINA_EVENTS_API_KEY")
            .or_else(|_| std::env::var("EVENTS_API_KEY"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        events_allow_query_key,
        webhook_secret,
        trust_proxy_headers,
        ephemeral_session_secret,
        auth_attempt_limiter: Arc::new(adapters::driving::web::auth::AuthAttemptLimiter::new()),
        edge_ingress_guard: Arc::new(adapters::driving::web::state::EdgeIngressGuard::from_env()),
        timezone: config.app.timezone.clone(),
        locale: config.app.locale.clone(),
        sim_jobs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        chat_queues: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        active_tasks: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
        presence_tracker: Arc::clone(&presence_tracker),
        workspace_dir: std::path::PathBuf::from(&config.agent.workspace_dir),
        brain_dir: crate::core::domain::AuditEngine::default_brain_path(),
        oidc_config: config.oidc.clone(),
        oidc_states: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
    });
    let app = create_router(state);

    let bind_addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("Aina Webhook server listening on http://{}", bind_addr);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    info!("Aina daemon shut down gracefully.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
