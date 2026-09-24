use crate::core::domain::{PresenceState, SessionRole};
use crate::core::ports::WhatsAppPort;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error, info, warn};

use std::sync::Arc;

pub struct WhatsmeowHttpAdapter {
    client: Client,
    base_url: String,
    api_key: String,
    send_endpoint: String,
    presence_endpoint: String,
    bot_session_id: Option<String>,
    companion_session_id: Option<String>,
    #[allow(dead_code)]
    companion_base_url: Option<String>,
    #[allow(dead_code)]
    companion_api_key: Option<String>,
    presence_tracker: Arc<crate::core::domain::PresenceTracker>,
}

impl WhatsmeowHttpAdapter {
    #[allow(dead_code)]
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        send_endpoint: impl Into<String>,
        presence_endpoint: impl Into<String>,
    ) -> Self {
        Self::with_sessions(
            base_url,
            api_key,
            send_endpoint,
            presence_endpoint,
            None,
            None,
        )
    }

    pub fn with_sessions(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        send_endpoint: impl Into<String>,
        presence_endpoint: impl Into<String>,
        bot_session_id: Option<String>,
        companion_session_id: Option<String>,
    ) -> Self {
        Self::with_companion_gateway(
            base_url,
            api_key,
            send_endpoint,
            presence_endpoint,
            bot_session_id,
            companion_session_id,
            None,
            None,
        )
    }

    pub fn with_companion_gateway(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        send_endpoint: impl Into<String>,
        presence_endpoint: impl Into<String>,
        bot_session_id: Option<String>,
        companion_session_id: Option<String>,
        companion_base_url: Option<String>,
        companion_api_key: Option<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            send_endpoint: send_endpoint.into(),
            presence_endpoint: presence_endpoint.into(),
            bot_session_id,
            companion_session_id,
            companion_base_url,
            companion_api_key,
            presence_tracker: Arc::new(crate::core::domain::PresenceTracker::new()),
        }
    }

    pub fn with_presence_tracker(mut self, tracker: Arc<crate::core::domain::PresenceTracker>) -> Self {
        self.presence_tracker = tracker;
        self
    }

    #[allow(dead_code)]
    pub fn presence_tracker(&self) -> &Arc<crate::core::domain::PresenceTracker> {
        &self.presence_tracker
    }

    fn resolve_session_id(&self, session_role: SessionRole) -> Option<&str> {
        match session_role {
            SessionRole::UserCompanion => self.companion_session_id.as_deref(),
            SessionRole::PrimaryBot => self.bot_session_id.as_deref(),
        }
    }

    async fn send_text_internal(
        &self,
        to_jid: &str,
        text: &str,
        quoted_id: Option<&str>,
        session_id: Option<&str>,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        // ALL outbound messages from Aina are STRICTLY sent via the PrimaryBot gateway (Aina's official bot number).
        // The companion account is purely a passive read-only sensor and must NEVER be used to send outbound chats.
        let base_url = self.base_url.as_str();
        let api_key = self.api_key.as_str();

        let is_status_broadcast = to_jid == "status@broadcast" || to_jid == "status";
        if is_status_broadcast {
            let clean = text.trim();
            let clean_lower = clean.to_lowercase();
            if clean.is_empty()
                || clean_lower.contains("error")
                || clean_lower.contains("exception")
                || clean_lower.contains("failed")
                || clean_lower.contains("traceback")
                || clean_lower.contains("500 internal")
                || clean_lower.contains("503 service")
                || clean_lower.contains("quota exceeded")
                || clean.starts_with("⚠️")
            {
                anyhow::bail!("Refused to post empty or error-laden message to status@broadcast: {}", clean);
            }
        }
        let url = if is_status_broadcast {
            format!("{}/api/v1/status/send-story", base_url.trim_end_matches('/'))
        } else {
            format!("{}{}", base_url.trim_end_matches('/'), self.send_endpoint)
        };
        
        let mut body = if is_status_broadcast {
            json!({
                "type": "text",
                "text": text,
            })
        } else {
            json!({
                "recipient": to_jid,
                "content": text,
                "to": to_jid,
                "receiver": to_jid,
                "chat_jid": to_jid,
                "message": text,
                "text": text,
            })
        };

        if let Some(sid) = session_id {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("session_id".to_string(), json!(sid));
                obj.insert("session".to_string(), json!(sid));
            }
        }

        if let Some(qid) = quoted_id {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("reply_to_id".to_string(), json!(qid));
                obj.insert("quoted_id".to_string(), json!(qid));
                obj.insert("quoted_message_id".to_string(), json!(qid));
            }
        }

        info!("Sending WhatsApp message to {} (role: {:?}, session: {:?})", to_jid, session_role, session_id);
        
        let max_attempts = 3;
        for attempt in 1..=max_attempts {
            let mut req = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("X-API-Key", api_key);

            if let Some(sid) = session_id {
                req = req.header("X-Session-ID", sid).header("Session-Id", sid);
            }

            let res = req.json(&body).send().await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    let body_text = resp.text().await.unwrap_or_default();
                    if status.is_success() {
                        debug!("WhatsApp message sent successfully: {}", body_text);
                        return Ok(());
                    } else if status == reqwest::StatusCode::NOT_FOUND {
                        let fallback_endpoint = if self.send_endpoint == "/api/v1/messages/send-text" {
                            "/send/message"
                        } else {
                            "/api/v1/messages/send-text"
                        };
                        let fallback_url = format!("{}{}", base_url.trim_end_matches('/'), fallback_endpoint);
                        debug!("Retrying with fallback endpoint: {}", fallback_url);
                        
                        let mut req_fallback = self
                            .client
                            .post(&fallback_url)
                            .header("Authorization", format!("Bearer {}", api_key))
                            .header("X-API-Key", api_key);

                        if let Some(sid) = session_id {
                            req_fallback = req_fallback.header("X-Session-ID", sid).header("Session-Id", sid);
                        }

                        let res_fallback = req_fallback.json(&body).send().await;
                        match res_fallback {
                            Ok(resp2) => {
                                if resp2.status().is_success() {
                                    return Ok(());
                                } else {
                                    let body2 = resp2.text().await.unwrap_or_default();
                                    error!("Fallback endpoint failed: {}", body2);
                                    anyhow::bail!("Whatsmeow HTTP error: {}", body2);
                                }
                            }
                            Err(e) => {
                                error!("Fallback connection failed: {}", e);
                                anyhow::bail!("Whatsmeow connection failed: {}", e);
                            }
                        }
                    } else if status.is_server_error() && attempt < max_attempts {
                        warn!(
                            "Send WhatsApp message attempt {}/{} failed with server error {} ({}). Retrying in {}s...",
                            attempt, max_attempts, status, body_text, attempt
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(attempt as u64)).await;
                        continue;
                    } else {
                        error!(
                            "Failed to send WhatsApp message. Status: {}, Body: {}",
                            status, body_text
                        );
                        anyhow::bail!("Whatsmeow HTTP error {}: {}", status, body_text);
                    }
                }
                Err(e) => {
                    if attempt < max_attempts {
                        warn!(
                            "Send WhatsApp message attempt {}/{} failed with connection error: {}. Retrying in {}s...",
                            attempt, max_attempts, e, attempt
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(attempt as u64)).await;
                        continue;
                    } else {
                        error!("Whatsmeow HTTP request connection error: {}", e);
                        anyhow::bail!("Whatsmeow connection failed: {}", e);
                    }
                }
            }
        }

        anyhow::bail!("Failed to send WhatsApp message after {} attempts", max_attempts);
    }

    async fn send_presence_internal(
        &self,
        to_jid: &str,
        state: PresenceState,
        session_id: Option<&str>,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        // ALL outbound presence indicators are sent via the PrimaryBot gateway.
        let base_url = self.base_url.as_str();
        let api_key = self.api_key.as_str();

        let url = format!("{}{}", base_url.trim_end_matches('/'), self.presence_endpoint);
        let state_str = match state {
            PresenceState::Composing => "composing",
            PresenceState::Paused => "paused",
        };

        let mut body = json!({
            "recipient": to_jid,
            "to": to_jid,
            "receiver": to_jid,
            "presence": state_str,
            "state": state_str,
        });

        if let Some(sid) = session_id {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("session_id".to_string(), json!(sid));
                obj.insert("session".to_string(), json!(sid));
            }
        }

        info!("Presence updated to '{}' for {} (role: {:?}, session: {:?})", state_str, to_jid, session_role, session_id);

        self.presence_tracker
            .record(to_jid, state_str, &format!("{:?}", session_role), "gateway_send")
            .await;

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("X-API-Key", api_key);

        if let Some(sid) = session_id {
            req = req.header("X-Session-ID", sid).header("Session-Id", sid);
        }

        let res = req.json(&body).send().await;

        if let Err(e) = res {
            warn!("Presence update failed (non-critical): {}", e);
        }

        Ok(())
    }

    async fn send_reaction_internal(
        &self,
        to_jid: &str,
        message_id: &str,
        emoji: &str,
        session_id: Option<&str>,
        _session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let base_url = self.base_url.as_str();
        let api_key = self.api_key.as_str();

        let primary_url = format!("{}/api/v1/messages/reaction", base_url.trim_end_matches('/'));
        let body = json!({
            "recipient": to_jid,
            "to": to_jid,
            "chat_jid": to_jid,
            "message_id": message_id,
            "reaction": emoji,
            "emoji": emoji
        });

        let mut req = self
            .client
            .post(&primary_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("X-API-Key", api_key);

        if let Some(sid) = session_id {
            req = req.header("X-Session-ID", sid).header("Session-Id", sid);
        }

        match req.json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                debug!("Reaction {} sent to {} on message {}", emoji, to_jid, message_id);
                Ok(())
            }
            Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
                let fallback_url = format!("{}/send/reaction", base_url.trim_end_matches('/'));
                let mut req_fb = self
                    .client
                    .post(&fallback_url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("X-API-Key", api_key);
                if let Some(sid) = session_id {
                    req_fb = req_fb.header("X-Session-ID", sid).header("Session-Id", sid);
                }
                let _ = req_fb.json(&body).send().await;
                Ok(())
            }
            Ok(resp) => {
                let status = resp.status();
                debug!("Whatsmeow reaction endpoint returned status {} (non-critical)", status);
                Ok(())
            }
            Err(e) => {
                debug!("Whatsmeow reaction connection failed (non-critical): {}", e);
                Ok(())
            }
        }
    }
}

#[async_trait]
impl WhatsAppPort for WhatsmeowHttpAdapter {
    async fn send_text(
        &self,
        to_jid: &str,
        text: &str,
        quoted_id: Option<&str>,
    ) -> anyhow::Result<()> {
        self.send_text_internal(to_jid, text, quoted_id, self.bot_session_id.as_deref(), SessionRole::PrimaryBot).await
    }

    async fn send_text_with_session(
        &self,
        to_jid: &str,
        text: &str,
        quoted_id: Option<&str>,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let session_id = self.resolve_session_id(session_role);
        self.send_text_internal(to_jid, text, quoted_id, session_id, session_role).await
    }

    async fn send_presence(&self, to_jid: &str, state: PresenceState) -> anyhow::Result<()> {
        self.send_presence_internal(to_jid, state, self.bot_session_id.as_deref(), SessionRole::PrimaryBot).await
    }

    async fn send_presence_with_session(
        &self,
        to_jid: &str,
        state: PresenceState,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let session_id = self.resolve_session_id(session_role);
        self.send_presence_internal(to_jid, state, session_id, session_role).await
    }

    async fn send_reaction(
        &self,
        to_jid: &str,
        message_id: &str,
        emoji: &str,
    ) -> anyhow::Result<()> {
        self.send_reaction_internal(to_jid, message_id, emoji, self.bot_session_id.as_deref(), SessionRole::PrimaryBot).await
    }

    async fn send_reaction_with_session(
        &self,
        to_jid: &str,
        message_id: &str,
        emoji: &str,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let session_id = self.resolve_session_id(session_role);
        self.send_reaction_internal(to_jid, message_id, emoji, session_id, session_role).await
    }
}
