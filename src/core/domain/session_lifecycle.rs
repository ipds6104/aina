//! Session lifecycle and inactivity management policy (SOLID: Single Responsibility Principle).

/// Default inactivity threshold in seconds before a conversation context expires (2 hours = 7200s).
pub const DEFAULT_SESSION_INACTIVITY_SECS: u64 = 7200;

/// Default maximum conversation turns before graceful session rollover (prevents massive context bloat).
pub const DEFAULT_MAX_SESSION_TURNS: u32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLifecyclePolicy {
    pub inactivity_timeout_secs: u64,
    pub max_turns: u32,
}

impl Default for SessionLifecyclePolicy {
    fn default() -> Self {
        Self {
            inactivity_timeout_secs: DEFAULT_SESSION_INACTIVITY_SECS,
            max_turns: DEFAULT_MAX_SESSION_TURNS,
        }
    }
}

#[allow(dead_code)]
impl SessionLifecyclePolicy {
    /// Creates a new policy with explicit inactivity timeout and default turn limit.
    pub fn new(inactivity_timeout_secs: u64) -> Self {
        Self {
            inactivity_timeout_secs,
            max_turns: DEFAULT_MAX_SESSION_TURNS,
        }
    }

    /// Creates a new policy with explicit inactivity timeout and turn limit.
    pub fn with_turns(inactivity_timeout_secs: u64, max_turns: u32) -> Self {
        Self {
            inactivity_timeout_secs,
            max_turns,
        }
    }

    /// Creates a policy reading `AINA_SESSION_INACTIVITY_SECS` and `AINA_SESSION_MAX_TURNS` from the environment.
    pub fn from_env() -> Self {
        let secs = std::env::var("AINA_SESSION_INACTIVITY_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_SESSION_INACTIVITY_SECS);

        let turns = std::env::var("AINA_SESSION_MAX_TURNS")
            .ok()
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(DEFAULT_MAX_SESSION_TURNS);

        Self {
            inactivity_timeout_secs: secs,
            max_turns: turns,
        }
    }

    /// Evaluates whether an existing session with the given elapsed seconds and turn count is still active.
    pub fn is_session_active(&self, elapsed_secs: u64, turn_count: u32) -> bool {
        elapsed_secs <= self.inactivity_timeout_secs && turn_count < self.max_turns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle_within_window_is_active() {
        let policy = SessionLifecyclePolicy::new(7200);
        assert!(policy.is_session_active(0, 1));
        assert!(policy.is_session_active(3600, 10));
        assert!(policy.is_session_active(7200, 39));
    }

    #[test]
    fn test_session_lifecycle_exceeding_window_is_expired() {
        let policy = SessionLifecyclePolicy::new(7200);
        assert!(!policy.is_session_active(7201, 5));
        assert!(!policy.is_session_active(10000, 5));
    }

    #[test]
    fn test_session_lifecycle_exceeding_turn_limit_is_expired() {
        let policy = SessionLifecyclePolicy::with_turns(7200, 40);
        assert!(!policy.is_session_active(100, 40));
        assert!(!policy.is_session_active(100, 45));
    }

    #[test]
    fn test_default_policy() {
        let policy = SessionLifecyclePolicy::default();
        assert_eq!(policy.inactivity_timeout_secs, 7200);
        assert_eq!(policy.max_turns, 40);
    }
}
