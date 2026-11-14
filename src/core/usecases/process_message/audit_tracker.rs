//! Action audit tracker and telemetry management for WhatsApp message processing.

use crate::core::domain::{AuditEngine, NewAgentToolExecution, NewWhatsAppActionAudit, UseCaseClassifier};
use crate::core::ports::SessionStorePort;
use std::sync::Arc;
use std::time::Instant;

pub struct AuditTracker {
    session_store: Arc<dyn SessionStorePort>,
    audit_id: Option<i64>,
    sender_jid: String,
    turn_token_prefix: String,
    sender_authority: String,
    input_text: String,
    has_media: bool,
    media_path: Option<String>,
    start_instant: Instant,
}

impl AuditTracker {
    /// Records an immediate terminal audit event (e.g. Ignore, RecordOnly, or handled Builtin command).
    pub async fn record_terminal_event(
        session_store: &Arc<dyn SessionStorePort>,
        message_id: String,
        chat_jid: String,
        sender_jid: String,
        sender_name: Option<String>,
        chat_type: &'static str,
        input_text: String,
        has_media: bool,
        media_path: Option<String>,
        decision: &str,
        decision_reason: &str,
        status: &str,
        response_text: Option<&str>,
        tool_name: Option<&str>,
        usecase_override: Option<&str>,
        duration_seconds: Option<f64>,
    ) -> anyhow::Result<()> {
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let initial_tools = tool_name.map(|t| vec![t.to_string()]).unwrap_or_default();
        let usecase = if let Some(u) = usecase_override {
            u.to_string()
        } else {
            UseCaseClassifier::classify(
                &input_text,
                has_media,
                media_path.as_deref(),
                &initial_tools,
            )
            .as_str()
            .to_string()
        };

        let audit = NewWhatsAppActionAudit {
            message_id,
            chat_jid,
            chat_type: chat_type.to_string(),
            sender_jid: sender_jid.clone(),
            sender_name,
            decision: decision.to_string(),
            decision_reason: decision_reason.to_string(),
            conversation_id: None,
            status: status.to_string(),
            input_text,
            has_media,
            media_path,
            response_text: response_text.map(|s| s.to_string()),
            error_message: None,
            duration_seconds,
            tools_invoked: initial_tools,
            usecase,
            created_at_epoch: now_epoch,
            completed_at_epoch: Some(now_epoch),
        };

        session_store.record_action_audit(&audit).await?;

        if let Some(t) = tool_name {
            let new_exec = NewAgentToolExecution {
                action_audit_id: None,
                turn_token_prefix: "tok_terminal".to_string(),
                sender_jid: sender_jid.clone(),
                sender_authority: "guest".to_string(),
                tool_name: t.to_string(),
                command_payload: "{}".to_string(),
                execution_status: status.to_string(),
                exit_code: if status == "success" { Some(0) } else { Some(1) },
                duration_ms: duration_seconds.map(|d| (d * 1000.0) as i64).unwrap_or(0),
                created_at_epoch: now_epoch,
            };
            let _ = session_store.record_tool_execution(&new_exec).await;
        }

        Ok(())
    }

    /// Initializes a tracking session for an in-progress response.
    pub async fn start_tracking(
        session_store: Arc<dyn SessionStorePort>,
        message_id: String,
        chat_jid: String,
        sender_jid: String,
        sender_name: Option<String>,
        chat_type: &'static str,
        input_text: String,
        has_media: bool,
        media_path: Option<String>,
        decision_reason: &str,
    ) -> Self {
        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let initial_usecase = UseCaseClassifier::classify(
            &input_text,
            has_media,
            media_path.as_deref(),
            &[],
        );

        let audit = NewWhatsAppActionAudit {
            message_id,
            chat_jid,
            chat_type: chat_type.to_string(),
            sender_jid: sender_jid.clone(),
            sender_name,
            decision: "respond".to_string(),
            decision_reason: decision_reason.to_string(),
            conversation_id: None,
            status: "in_progress".to_string(),
            input_text: input_text.clone(),
            has_media,
            media_path: media_path.clone(),
            response_text: None,
            error_message: None,
            duration_seconds: None,
            tools_invoked: vec![],
            usecase: initial_usecase.as_str().to_string(),
            created_at_epoch: now_epoch,
            completed_at_epoch: None,
        };

        let audit_id = session_store.record_action_audit(&audit).await.ok();

        Self {
            session_store,
            audit_id,
            sender_jid,
            turn_token_prefix: "tok_initial".to_string(),
            sender_authority: "guest".to_string(),
            input_text,
            has_media,
            media_path,
            start_instant: Instant::now(),
        }
    }

    /// Attaches the verified turn capability to attribute tool executions to the verified principal.
    pub fn attach_capability(&mut self, cap: &crate::core::domain::capability::TurnCapability) {
        self.turn_token_prefix = cap.token.chars().take(12).collect();
        self.sender_authority = cap.authority.as_str().to_string();
    }

    /// Records completion of a handled built-in command or conversational reaction.
    pub async fn complete_handled(
        &self,
        response_text: &str,
        tool_name: &str,
        usecase: &str,
        status: &str,
        error_detail: Option<&str>,
    ) {
        let dur = self.start_instant.elapsed().as_secs_f64();
        if let Some(aid) = self.audit_id {
            let _ = self
                .session_store
                .update_action_audit_result(
                    aid,
                    None,
                    Some(response_text),
                    error_detail,
                    status,
                    Some(dur),
                    &[tool_name.to_string()],
                    Some(usecase),
                )
                .await;
        }

        let new_exec = NewAgentToolExecution {
            action_audit_id: self.audit_id,
            turn_token_prefix: self.turn_token_prefix.clone(),
            sender_jid: self.sender_jid.clone(),
            sender_authority: self.sender_authority.clone(),
            tool_name: tool_name.to_string(),
            command_payload: "{}".to_string(),
            execution_status: status.to_string(),
            exit_code: if status == "success" { Some(0) } else { Some(1) },
            duration_ms: (dur * 1000.0) as i64,
            created_at_epoch: chrono::Utc::now().timestamp(),
        };
        let _ = self.session_store.record_tool_execution(&new_exec).await;
    }

    /// Finalizes the audit record upon successful agent execution.
    pub async fn complete_success(
        &self,
        conversation_id: &str,
        response_text: &str,
    ) {
        let dur = self.start_instant.elapsed().as_secs_f64();
        let tools_invoked = AuditEngine::extract_tools_for_conversation(
            &AuditEngine::default_brain_path(),
            conversation_id,
        );
        let refined_usecase = UseCaseClassifier::classify(
            &self.input_text,
            self.has_media,
            self.media_path.as_deref(),
            &tools_invoked,
        );

        if let Some(aid) = self.audit_id {
            let _ = self
                .session_store
                .update_action_audit_result(
                    aid,
                    Some(conversation_id),
                    Some(response_text),
                    None,
                    "success",
                    Some(dur),
                    &tools_invoked,
                    Some(refined_usecase.as_str()),
                )
                .await;
        }

        for tool in &tools_invoked {
            let new_exec = NewAgentToolExecution {
                action_audit_id: self.audit_id,
                turn_token_prefix: self.turn_token_prefix.clone(),
                sender_jid: self.sender_jid.clone(),
                sender_authority: self.sender_authority.clone(),
                tool_name: tool.clone(),
                command_payload: "{}".to_string(),
                execution_status: "success".to_string(),
                exit_code: Some(0),
                duration_ms: (dur * 1000.0) as i64,
                created_at_epoch: chrono::Utc::now().timestamp(),
            };
            let _ = self.session_store.record_tool_execution(&new_exec).await;
        }
    }

    /// Finalizes the audit record upon execution failure.
    pub async fn complete_failure(
        &self,
        raw_err: &str,
        existing_conv_id: Option<&str>,
    ) {
        let dur = self.start_instant.elapsed().as_secs_f64();
        let partial_tools = if let Some(cid) = existing_conv_id {
            AuditEngine::extract_tools_for_conversation(
                &AuditEngine::default_brain_path(),
                cid,
            )
        } else {
            vec![]
        };

        let refined_usecase = UseCaseClassifier::classify(
            &self.input_text,
            self.has_media,
            self.media_path.as_deref(),
            &partial_tools,
        );

        if let Some(aid) = self.audit_id {
            let _ = self
                .session_store
                .update_action_audit_result(
                    aid,
                    None,
                    None,
                    Some(raw_err),
                    "failed",
                    Some(dur),
                    &partial_tools,
                    Some(refined_usecase.as_str()),
                )
                .await;
        }

        for tool in &partial_tools {
            let new_exec = NewAgentToolExecution {
                action_audit_id: self.audit_id,
                turn_token_prefix: self.turn_token_prefix.clone(),
                sender_jid: self.sender_jid.clone(),
                sender_authority: self.sender_authority.clone(),
                tool_name: tool.clone(),
                command_payload: "{}".to_string(),
                execution_status: "failed".to_string(),
                exit_code: Some(1),
                duration_ms: (dur * 1000.0) as i64,
                created_at_epoch: chrono::Utc::now().timestamp(),
            };
            let _ = self.session_store.record_tool_execution(&new_exec).await;
        }
    }
}
