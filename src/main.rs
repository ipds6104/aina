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
    info!(
        "Loaded persona with {} characters, organization context with {} characters",
        persona_text.len(),
        org_text.len()
    );

    // 2. Instantiate Driven Adapters (Secondary Adapters)
    let session_store = Arc::new(SqliteSessionStore::new(&config.database.path)?);
    let agent_engine = Arc::new(AntigravityCliAdapter::new(
        &config.agent.binary_path,
        &config.agent.model,
        &config.agent.workspace_dir,
        config.agent.timeout_seconds,
    ));
    let whatsapp = Arc::new(WhatsmeowHttpAdapter::with_sessions(
        &config.whatsmeow.base_url,
        &config.whatsmeow.api_key,
        &config.whatsmeow.send_endpoint,
        &config.whatsmeow.presence_endpoint,
        config.whatsmeow.bot_session_id.clone(),
        config.whatsmeow.companion_session_id.clone(),
    ));

    let persona_engine = Arc::new(PersonaEngine::new(
        persona_text,
        org_text,
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
        Some(std::path::PathBuf::from(&config.agent.workspace_dir)),
    ));

    // 4. Start Scheduler if enabled (Driving Adapter)
    if config.scheduler.enabled {
        let runner = SchedulerRunner::new(
            Arc::clone(&scheduled_tick_usecase),
            config.scheduler.interval_seconds,
        );
        runner.start();
    }

    // 5. Setup Code / Admin Key for First-Time Setup Security
    let setup_code = std::env::var("ADMIN_KEY")
        .or_else(|_| std::env::var("AINA_ADMIN_KEY"))
        .unwrap_or_else(|_| {
            let code_num: u32 = rand::random::<u32>() % 900000 + 100000;
            format!("AINA-{}", code_num)
        });

    info!("========================================================================");
    info!("🔐 SETUP / ADMIN CODE: {}", setup_code);
    info!("Use this code in the Web Setup Wizard (/setup) to claim or update Aina.");
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
        setup_code,
        timezone: config.app.timezone.clone(),
        locale: config.app.locale.clone(),
        sim_jobs: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        chat_queues: Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
    });
    let app = create_router(state);

    let bind_addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("Aina Webhook server listening on http://{}", bind_addr);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app)
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
