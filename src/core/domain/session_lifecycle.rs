//! Session lifecycle and inactivity management policy (SOLID: Single Responsibility Principle).

/// Default inactivity threshold in seconds before a conversation context expires (2 hours = 7200s).
pub const DEFAULT_SESSION_INACTIVITY_SECS: u64 = 7200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionLifecyclePolicy {
    pub inactivity_timeout_secs: u64,
}

impl Default for SessionLifecyclePolicy {
    fn default() -> Self {
        Self {
            inactivity_timeout_secs: DEFAULT_SESSION_INACTIVITY_SECS,
        }
    }
}

#[allow(dead_code)]
impl SessionLifecyclePolicy {
    /// Creates a new policy with explicit inactivity timeout.
    pub fn new(inactivity_timeout_secs: u64) -> Self {
        Self {
            inactivity_timeout_secs,
        }
    }

    /// Creates a policy reading `AINA_SESSION_INACTIVITY_SECS` from the environment if present,
    /// falling back to the default 7200 seconds (2 hours).
    pub fn from_env() -> Self {
        let secs = std::env::var("AINA_SESSION_INACTIVITY_SECS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_SESSION_INACTIVITY_SECS);

        Self {
            inactivity_timeout_secs: secs,
        }
    }

    /// Evaluates whether an existing session with the given elapsed seconds since last activity is still active.
    pub fn is_session_active(&self, elapsed_secs: u64) -> bool {
        elapsed_secs <= self.inactivity_timeout_secs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle_within_window_is_active() {
        let policy = SessionLifecyclePolicy::new(7200);
        assert!(policy.is_session_active(0));
        assert!(policy.is_session_active(3600));
        assert!(policy.is_session_active(7200));
    }

    #[test]
    fn test_session_lifecycle_exceeding_window_is_expired() {
        let policy = SessionLifecyclePolicy::new(7200);
        assert!(!policy.is_session_active(7201));
        assert!(!policy.is_session_active(10000));
    }

    #[test]
    fn test_default_policy() {
        let policy = SessionLifecyclePolicy::default();
        assert_eq!(policy.inactivity_timeout_secs, 7200);
    }
}
