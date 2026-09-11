use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub whatsmeow: WhatsmeowConfig,
    pub agent: AgentConfig,
    pub scheduler: SchedulerConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WhatsmeowConfig {
    pub base_url: String,
    pub api_key: String,
    pub bot_jid: String,
    pub bot_name: String,
    #[serde(default = "default_send_endpoint")]
    pub send_endpoint: String,
    #[serde(default = "default_presence_endpoint")]
    pub presence_endpoint: String,
}

fn default_send_endpoint() -> String {
    "/send/message".to_string()
}

fn default_presence_endpoint() -> String {
    "/send/presence".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub binary_path: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_workspace")]
    pub workspace_dir: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default = "default_persona_file")]
    pub persona_file: String,
}

fn default_model() -> String {
    "gemini-3.8-flash-high".to_string()
}

fn default_workspace() -> String {
    "./workspace".to_string()
}

fn default_timeout() -> u64 {
    300
}

fn default_persona_file() -> String {
    "config/persona.md".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_scheduler_enabled")]
    pub enabled: bool,
    #[serde(default = "default_scheduler_interval")]
    pub interval_seconds: u64,
}

fn default_scheduler_enabled() -> bool {
    true
}

fn default_scheduler_interval() -> u64 {
    60
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

fn default_db_path() -> String {
    "data/aina.db".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8090,
            },
            whatsmeow: WhatsmeowConfig {
                base_url: "http://localhost:3000".to_string(),
                api_key: "default-secret".to_string(),
                bot_jid: "628123456789@s.whatsapp.net".to_string(),
                bot_name: "Aina".to_string(),
                send_endpoint: default_send_endpoint(),
                presence_endpoint: default_presence_endpoint(),
            },
            agent: AgentConfig {
                binary_path: "/home/ihza/.local/bin/agy".to_string(),
                model: default_model(),
                workspace_dir: default_workspace(),
                timeout_seconds: default_timeout(),
                persona_file: default_persona_file(),
            },
            scheduler: SchedulerConfig {
                enabled: default_scheduler_enabled(),
                interval_seconds: default_scheduler_interval(),
            },
            database: DatabaseConfig {
                path: default_db_path(),
            },
        }
    }
}

impl AppConfig {
    pub fn load_from_file_or_default<P: AsRef<Path>>(path: P) -> Self {
        let mut config = if let Ok(content) = fs::read_to_string(path) {
            serde_yaml::from_str(&content).unwrap_or_default()
        } else {
            AppConfig::default()
        };

        config.apply_env_overrides();
        config
    }

    /// Overrides configuration parameters using Environment Variables (ideal for Coolify/Docker)
    pub fn apply_env_overrides(&mut self) {
        // Server overrides (Coolify often injects PORT)
        if let Ok(port_str) = env::var("PORT").or_else(|_| env::var("SERVER_PORT")) {
            if let Ok(p) = port_str.parse::<u16>() {
                self.server.port = p;
            }
        }
        if let Ok(host) = env::var("SERVER_HOST") {
            self.server.host = host;
        }

        // Whatsmeow overrides
        if let Ok(val) = env::var("WHATSMEOW_BASE_URL") {
            self.whatsmeow.base_url = val;
        }
        if let Ok(val) = env::var("WHATSMEOW_API_KEY") {
            self.whatsmeow.api_key = val;
        }
        if let Ok(val) = env::var("WHATSMEOW_BOT_JID") {
            self.whatsmeow.bot_jid = val;
        }
        if let Ok(val) = env::var("WHATSMEOW_BOT_NAME") {
            self.whatsmeow.bot_name = val;
        }
        if let Ok(val) = env::var("WHATSMEOW_SEND_ENDPOINT") {
            self.whatsmeow.send_endpoint = val;
        }
        if let Ok(val) = env::var("WHATSMEOW_PRESENCE_ENDPOINT") {
            self.whatsmeow.presence_endpoint = val;
        }

        // Agent overrides
        if let Ok(val) = env::var("AGENT_BINARY_PATH").or_else(|_| env::var("AGY_BINARY_PATH")) {
            self.agent.binary_path = val;
        }
        if let Ok(val) = env::var("AGENT_MODEL") {
            self.agent.model = val;
        }
        if let Ok(val) = env::var("AGENT_WORKSPACE") {
            self.agent.workspace_dir = val;
        }
        if let Ok(val) = env::var("AGENT_TIMEOUT_SECONDS") {
            if let Ok(t) = val.parse::<u64>() {
                self.agent.timeout_seconds = t;
            }
        }
        if let Ok(val) = env::var("AGENT_PERSONA_FILE") {
            self.agent.persona_file = val;
        }

        // Scheduler overrides
        if let Ok(val) = env::var("SCHEDULER_ENABLED") {
            self.scheduler.enabled = val.to_lowercase() == "true" || val == "1";
        }
        if let Ok(val) = env::var("SCHEDULER_INTERVAL_SECONDS") {
            if let Ok(i) = val.parse::<u64>() {
                self.scheduler.interval_seconds = i;
            }
        }

        // Database overrides
        if let Ok(val) = env::var("DATABASE_PATH") {
            self.database.path = val;
        }
    }

    pub fn load_persona(&self) -> String {
        if let Ok(persona_override) = env::var("AINA_PERSONA_TEXT") {
            if !persona_override.trim().is_empty() {
                return persona_override;
            }
        }

        fs::read_to_string(&self.agent.persona_file).unwrap_or_else(|_| {
            "Kamu adalah Aina, asisten dan rekan kerja cerdas yang ramah, cekatan, dan solutif."
                .to_string()
        })
    }
}
