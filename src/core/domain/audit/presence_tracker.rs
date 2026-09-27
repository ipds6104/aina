//! In-memory thread-safe tracker for active composing presence and recent history.

use super::models::{ActiveTypingStatus, PresenceAuditRecord, PresenceAuditSnapshot};
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct PresenceTracker {
    active: RwLock<HashMap<String, ActiveTypingStatus>>,
    history: RwLock<VecDeque<PresenceAuditRecord>>,
    max_history: usize,
}

impl Default for PresenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PresenceTracker {
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    pub fn with_capacity(max_history: usize) -> Self {
        Self {
            active: RwLock::new(HashMap::new()),
            history: RwLock::new(VecDeque::with_capacity(max_history)),
            max_history,
        }
    }

    pub async fn record(&self, chat_jid: &str, state: &str, session_role: &str, trigger: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        if state == "composing" {
            let mut active = self.active.write().await;
            let entry = active.entry(chat_jid.to_string()).or_insert_with(|| ActiveTypingStatus {
                chat_jid: chat_jid.to_string(),
                session_role: session_role.to_string(),
                started_at_epoch: now,
                last_beat_epoch: now,
                duration_seconds: 0,
                heartbeat_count: 0,
            });
            entry.last_beat_epoch = now;
            entry.duration_seconds = (now - entry.started_at_epoch).max(0);
            entry.heartbeat_count += 1;
        } else {
            let mut active = self.active.write().await;
            active.remove(chat_jid);
        }

        let mut history = self.history.write().await;
        if history.len() >= self.max_history {
            history.pop_front();
        }
        history.push_back(PresenceAuditRecord {
            timestamp_epoch: now,
            chat_jid: chat_jid.to_string(),
            state: state.to_string(),
            session_role: session_role.to_string(),
            trigger: trigger.to_string(),
        });
    }

    pub async fn snapshot(&self) -> PresenceAuditSnapshot {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let active_map = self.active.read().await;
        let mut active_list: Vec<ActiveTypingStatus> = active_map.values().cloned().collect();
        for item in &mut active_list {
            item.duration_seconds = (now - item.started_at_epoch).max(0);
        }
        active_list.sort_by(|a, b| b.started_at_epoch.cmp(&a.started_at_epoch));

        let history = self.history.read().await;
        let recent_events: Vec<PresenceAuditRecord> = history.iter().rev().cloned().collect();

        PresenceAuditSnapshot {
            active_typing_count: active_list.len(),
            active_typing: active_list,
            recent_events,
        }
    }

    pub async fn clear_active(&self, chat_jid: Option<&str>) -> Vec<String> {
        let mut active = self.active.write().await;
        if let Some(jid) = chat_jid {
            if active.remove(jid).is_some() {
                vec![jid.to_string()]
            } else {
                vec![]
            }
        } else {
            let jids: Vec<String> = active.keys().cloned().collect();
            active.clear();
            jids
        }
    }
}
