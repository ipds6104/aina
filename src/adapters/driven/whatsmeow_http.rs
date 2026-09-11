use crate::core::domain::PresenceState;
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
}

impl WhatsmeowHttpAdapter {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        send_endpoint: impl Into<String>,
        presence_endpoint: impl Into<String>,
    ) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            api_key: api_key.into(),
            send_endpoint: send_endpoint.into(),
            presence_endpoint: presence_endpoint.into(),
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
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), self.send_endpoint);
        
        let mut body = json!({
            "to": to_jid,
            "receiver": to_jid,
            "chat_jid": to_jid,
            "message": text,
            "text": text,
        });

        if let Some(qid) = quoted_id {
            if let Some(obj) = body.as_object_mut() {
                obj.insert("quoted_id".to_string(), json!(qid));
                obj.insert("quoted_message_id".to_string(), json!(qid));
            }
        }

        info!("Sending WhatsApp message to {}", to_jid);
        
        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-API-Key", &self.api_key)
            .json(&body)
            .send()
            .await;

        match res {
            Ok(resp) => {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    debug!("WhatsApp message sent successfully: {}", body_text);
                    Ok(())
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

    async fn send_presence(&self, to_jid: &str, state: PresenceState) -> anyhow::Result<()> {
        let url = format!("{}{}", self.base_url.trim_end_matches('/'), self.presence_endpoint);
        let state_str = match state {
            PresenceState::Composing => "composing",
            PresenceState::Paused => "paused",
        };

        let body = json!({
            "to": to_jid,
            "receiver": to_jid,
            "presence": state_str,
            "state": state_str,
        });

        debug!("Sending presence '{}' to {}", state_str, to_jid);

        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("X-API-Key", &self.api_key)
            .json(&body)
            .send()
            .await;

        if let Err(e) = res {
            debug!("Presence update failed (non-critical): {}", e);
        }

        Ok(())
    }
}
