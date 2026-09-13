use crate::core::domain::{PresenceState, SessionRole};
use crate::core::ports::WhatsAppPort;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error, info};

pub struct WhatsmeowHttpAdapter {
    client: Client,
    base_url: String,
    api_key: String,
    send_endpoint: String,
    presence_endpoint: String,
    bot_session_id: Option<String>,
    companion_session_id: Option<String>,
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
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            send_endpoint: send_endpoint.into(),
            presence_endpoint: presence_endpoint.into(),
            bot_session_id,
            companion_session_id,
        }
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
    ) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), self.send_endpoint);
        
        let mut body = json!({
            "recipient": to_jid,
            "content": text,
            "to": to_jid,
            "receiver": to_jid,
            "chat_jid": to_jid,
            "message": text,
            "text": text,
        });

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

        info!("Sending WhatsApp message to {} (session: {:?})", to_jid, session_id);
        
        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-API-Key", &self.api_key);

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
                    Ok(())
                } else if status == reqwest::StatusCode::NOT_FOUND {
                    let fallback_endpoint = if self.send_endpoint == "/api/v1/messages/send-text" {
                        "/send/message"
                    } else {
                        "/api/v1/messages/send-text"
                    };
                    let fallback_url = format!("{}{}", self.base_url.trim_end_matches('/'), fallback_endpoint);
                    info!("Endpoint {} returned 404, falling back to {}", url, fallback_url);
                    let mut fb_req = self
                        .client
                        .post(&fallback_url)
                        .header("Authorization", format!("Bearer {}", self.api_key))
                        .header("X-API-Key", &self.api_key);

                    if let Some(sid) = session_id {
                        fb_req = fb_req.header("X-Session-ID", sid).header("Session-Id", sid);
                    }

                    let fb_res = fb_req.json(&body).send().await;
                    match fb_res {
                        Ok(fb_resp) => {
                            let fb_status = fb_resp.status();
                            let fb_body = fb_resp.text().await.unwrap_or_default();
                            if fb_status.is_success() {
                                debug!("WhatsApp message sent successfully via fallback: {}", fb_body);
                                Ok(())
                            } else {
                                error!("Fallback endpoint also failed. Status: {}, Body: {}", fb_status, fb_body);
                                anyhow::bail!("Whatsmeow HTTP error {}: {}", fb_status, fb_body);
                            }
                        }
                        Err(e) => {
                            error!("Fallback connection error: {}", e);
                            anyhow::bail!("Whatsmeow connection failed: {}", e);
                        }
                    }
                } else {
                    error!(
                        "Failed to send WhatsApp message. Status: {}, Body: {}",
                        status, body_text
                    );
                    anyhow::bail!("Whatsmeow HTTP error {}: {}", status, body_text);
                }
            }
            Err(e) => {
                error!("Whatsmeow HTTP request connection error: {}", e);
                anyhow::bail!("Whatsmeow connection failed: {}", e);
            }
        }
    }

    async fn send_presence_internal(
        &self,
        to_jid: &str,
        state: PresenceState,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), self.presence_endpoint);
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

        debug!("Sending presence '{}' to {} (session: {:?})", state_str, to_jid, session_id);

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-API-Key", &self.api_key);

        if let Some(sid) = session_id {
            req = req.header("X-Session-ID", sid).header("Session-Id", sid);
        }

        let res = req.json(&body).send().await;

        if let Err(e) = res {
            debug!("Presence update failed (non-critical): {}", e);
        }

        Ok(())
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
        self.send_text_internal(to_jid, text, quoted_id, self.bot_session_id.as_deref()).await
    }

    async fn send_text_with_session(
        &self,
        to_jid: &str,
        text: &str,
        quoted_id: Option<&str>,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let session_id = self.resolve_session_id(session_role);
        self.send_text_internal(to_jid, text, quoted_id, session_id).await
    }

    async fn send_presence(&self, to_jid: &str, state: PresenceState) -> anyhow::Result<()> {
        self.send_presence_internal(to_jid, state, self.bot_session_id.as_deref()).await
    }

    async fn send_presence_with_session(
        &self,
        to_jid: &str,
        state: PresenceState,
        session_role: SessionRole,
    ) -> anyhow::Result<()> {
        let session_id = self.resolve_session_id(session_role);
        self.send_presence_internal(to_jid, state, session_id).await
    }
}
