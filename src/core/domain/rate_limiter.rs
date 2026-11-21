//! Pure hexagonal domain ingress rate limiter (ADR-001, ADR-018).
//!
//! Zero async runtime, zero I/O, zero wall-clock calls inside domain logic:
//! every evaluation accepts an explicit `now_epoch: i64` so behavior is 100%
//! deterministic and unit-testable.
//!
//! Anti-ban invariant (WhatsApp):
//! - When a sender exhausts their authority-tier token bucket, emit at most
//!   ONE polite cooldown notice per cooldown window (`NotifyOnceAndDrop`).
//! - Subsequent burst messages in the same window are silently dropped
//!   (`SilentDrop`, zero outbound WhatsApp messages).
//! - Persistent burst abuse (>= `tombstone_threshold` consecutive violations)
//!   activates a temporary silent isolation window (`tombstone_secs`, default 900s).

use crate::core::domain::capability::Authority;
use std::collections::HashMap;

/// Default maximum number of tracked sender buckets before pruning/eviction.
pub const DEFAULT_MAX_BUCKETS: usize = 2_000;
/// Idle buckets older than this threshold are evicted during `prune_expired`.
pub const DEFAULT_BUCKET_IDLE_TTL_SECS: i64 = 1_800;

/// Token bucket parameters for a single authority tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierQuota {
    /// Maximum burst tokens available in the bucket.
    pub capacity: u32,
    /// Seconds required to refill 1 token.
    pub refill_interval_secs: u64,
    /// Minimum cooldown floor (seconds) enforced after bucket depletion.
    pub min_gap_secs: u64,
}

/// Authority-aware rate limit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub guest: TierQuota,
    pub staff: TierQuota,
    /// Consecutive rejected attempts that trigger a temporary silent tombstone.
    pub tombstone_threshold: u32,
    /// Duration of silent isolation once tombstoned (seconds).
    pub tombstone_secs: u64,
    /// Hard upper bound on tracked sender entries in memory.
    pub max_buckets: usize,
    /// Idle TTL for pruning inactive buckets (seconds).
    pub idle_ttl_secs: i64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            guest: TierQuota {
                capacity: 3,
                refill_interval_secs: 20,
                min_gap_secs: 5,
            },
            staff: TierQuota {
                capacity: 10,
                refill_interval_secs: 6,
                min_gap_secs: 0,
            },
            tombstone_threshold: 5,
            tombstone_secs: 900,
            max_buckets: DEFAULT_MAX_BUCKETS,
            idle_ttl_secs: DEFAULT_BUCKET_IDLE_TTL_SECS,
        }
    }
}

impl RateLimitConfig {
    /// Loads optional overrides from environment variables while preserving safe defaults.
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Some(v) = parse_env_u32("AINA_RL_GUEST_CAPACITY").filter(|&n| n >= 1) {
            cfg.guest.capacity = v;
        }
        if let Some(v) = parse_env_u64("AINA_RL_GUEST_REFILL_SECS").filter(|&n| n >= 1) {
            cfg.guest.refill_interval_secs = v;
        }
        if let Some(v) = parse_env_u64("AINA_RL_GUEST_MIN_GAP_SECS") {
            cfg.guest.min_gap_secs = v;
        }
        if let Some(v) = parse_env_u32("AINA_RL_STAFF_CAPACITY").filter(|&n| n >= 1) {
            cfg.staff.capacity = v;
        }
        if let Some(v) = parse_env_u64("AINA_RL_STAFF_REFILL_SECS").filter(|&n| n >= 1) {
            cfg.staff.refill_interval_secs = v;
        }
        if let Some(v) = parse_env_u32("AINA_RL_TOMBSTONE_THRESHOLD").filter(|&n| n >= 1) {
            cfg.tombstone_threshold = v;
        }
        if let Some(v) = parse_env_u64("AINA_RL_TOMBSTONE_SECS").filter(|&n| n >= 1) {
            cfg.tombstone_secs = v;
        }

        cfg
    }
}

fn parse_env_u32(key: &str) -> Option<u32> {
    std::env::var(key).ok()?.trim().parse::<u32>().ok()
}

fn parse_env_u64(key: &str) -> Option<u64> {
    std::env::var(key).ok()?.trim().parse::<u64>().ok()
}

/// Deterministic decision returned by [`IngressRateLimiter::check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThrottleDecision {
    /// Sender is within quota; proceed to agent execution.
    Allow,
    /// Quota exceeded for the first time in the current cooldown window:
    /// send at most ONE polite cooldown notice and drop this message.
    NotifyOnceAndDrop { retry_after_secs: u64 },
    /// Subsequent burst or active tombstone: drop silently (0 WhatsApp replies).
    SilentDrop {
        retry_after_secs: u64,
        consecutive_hits: u32,
        tombstoned: bool,
    },
}

#[derive(Debug, Clone)]
struct SenderBucket {
    tokens: u32,
    last_refill_epoch: i64,
    last_allowed_epoch: i64,
    last_seen_epoch: i64,
    burst_exhausted: bool,
    notified_in_window: bool,
    consecutive_hits: u32,
    tombstone_until: Option<i64>,
}

/// Pure in-memory authority-aware ingress rate limiter with bounded memory.
#[derive(Debug)]
pub struct IngressRateLimiter {
    config: RateLimitConfig,
    buckets: HashMap<String, SenderBucket>,
}

impl Default for IngressRateLimiter {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

impl IngressRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            buckets: HashMap::new(),
        }
    }

    pub fn config(&self) -> RateLimitConfig {
        self.config
    }

    #[cfg(test)]
    pub fn tracked_senders(&self) -> usize {
        self.buckets.len()
    }

    /// Evaluates whether `sender_jid` with verified `authority` may proceed at `now_epoch`.
    pub fn check(
        &mut self,
        sender_jid: &str,
        authority: Authority,
        now_epoch: i64,
    ) -> ThrottleDecision {
        // Root of Trust (Admin) always bypasses rate limiting.
        if authority == Authority::Admin {
            return ThrottleDecision::Allow;
        }

        let quota = match authority {
            Authority::Admin => return ThrottleDecision::Allow,
            Authority::Atasan | Authority::Staff => self.config.staff,
            Authority::Guest => self.config.guest,
        };

        let key = normalize_sender_key(sender_jid);

        if !self.buckets.contains_key(&key) && self.buckets.len() >= self.config.max_buckets {
            self.prune_expired(now_epoch);
        }

        let bucket = self.buckets.entry(key).or_insert_with(|| SenderBucket {
            tokens: quota.capacity,
            last_refill_epoch: now_epoch,
            last_allowed_epoch: now_epoch.saturating_sub(quota.min_gap_secs as i64),
            last_seen_epoch: now_epoch,
            burst_exhausted: false,
            notified_in_window: false,
            consecutive_hits: 0,
            tombstone_until: None,
        });

        bucket.last_seen_epoch = now_epoch;

        // 1. Active tombstone check (strict silent drop regardless of elapsed token refill).
        if let Some(until) = bucket.tombstone_until {
            if now_epoch < until {
                bucket.consecutive_hits = bucket.consecutive_hits.saturating_add(1);
                let retry_after = (until - now_epoch).max(1) as u64;
                return ThrottleDecision::SilentDrop {
                    retry_after_secs: retry_after,
                    consecutive_hits: bucket.consecutive_hits,
                    tombstoned: true,
                };
            } else {
                // Tombstone expired: clean reset for the new window.
                bucket.tombstone_until = None;
                bucket.consecutive_hits = 0;
                bucket.notified_in_window = false;
                bucket.burst_exhausted = false;
                bucket.tokens = quota.capacity.min(1);
                bucket.last_refill_epoch = now_epoch;
            }
        }

        // 2. Refill tokens deterministically based on elapsed seconds.
        let refill_secs = quota.refill_interval_secs.max(1) as i64;
        if now_epoch > bucket.last_refill_epoch {
            let elapsed = now_epoch - bucket.last_refill_epoch;
            let gained = (elapsed / refill_secs) as u64;
            if gained > 0 {
                let new_tokens = (bucket.tokens as u64).saturating_add(gained);
                bucket.tokens = new_tokens.min(quota.capacity as u64) as u32;
                if bucket.tokens >= quota.capacity {
                    bucket.last_refill_epoch = now_epoch;
                } else {
                    bucket.last_refill_epoch =
                        bucket.last_refill_epoch.saturating_add((gained as i64).saturating_mul(refill_secs));
                }
            }
        } else if bucket.tokens > quota.capacity {
            bucket.tokens = quota.capacity;
        }

        // 3. Enforce post-burst minimum gap floor if burst was previously exhausted.
        let min_gap = quota.min_gap_secs as i64;
        let gap_remaining = if bucket.burst_exhausted && min_gap > 0 {
            let elapsed_since_allowed = now_epoch - bucket.last_allowed_epoch;
            if elapsed_since_allowed < min_gap {
                (min_gap - elapsed_since_allowed) as u64
            } else {
                0
            }
        } else {
            0
        };

        // 4. Consume a token if available and min-gap floor is satisfied.
        if bucket.tokens >= 1 && gap_remaining == 0 {
            bucket.tokens -= 1;
            bucket.last_allowed_epoch = now_epoch;
            bucket.consecutive_hits = 0;
            bucket.notified_in_window = false;
            if bucket.tokens == 0 {
                bucket.burst_exhausted = true;
            } else {
                bucket.burst_exhausted = false;
            }
            return ThrottleDecision::Allow;
        }

        // 5. Rate-limited path: compute retry_after, track violations, enforce 1x notice + tombstone.
        bucket.consecutive_hits = bucket.consecutive_hits.saturating_add(1);

        let refill_wait = if bucket.tokens == 0 {
            let elapsed_since_refill = (now_epoch - bucket.last_refill_epoch).max(0);
            (refill_secs - elapsed_since_refill).max(1) as u64
        } else {
            0
        };
        let retry_after_secs = refill_wait.max(gap_remaining).max(quota.min_gap_secs).max(1);

        if bucket.consecutive_hits >= self.config.tombstone_threshold {
            let tombstone_secs = self.config.tombstone_secs.max(1);
            bucket.tombstone_until = Some(now_epoch.saturating_add(tombstone_secs as i64));
            bucket.notified_in_window = true;
            return ThrottleDecision::SilentDrop {
                retry_after_secs: tombstone_secs,
                consecutive_hits: bucket.consecutive_hits,
                tombstoned: true,
            };
        }

        if !bucket.notified_in_window {
            bucket.notified_in_window = true;
            ThrottleDecision::NotifyOnceAndDrop { retry_after_secs }
        } else {
            ThrottleDecision::SilentDrop {
                retry_after_secs,
                consecutive_hits: bucket.consecutive_hits,
                tombstoned: false,
            }
        }
    }

    /// Removes idle/expired buckets and enforces `config.max_buckets` hard cap to prevent OOM.
    pub fn prune_expired(&mut self, now_epoch: i64) -> usize {
        let before = self.buckets.len();
        let idle_ttl = self.config.idle_ttl_secs;

        self.buckets.retain(|_, b| {
            let tombstoned_active = b.tombstone_until.map(|u| now_epoch < u).unwrap_or(false);
            if tombstoned_active {
                return true;
            }
            now_epoch.saturating_sub(b.last_seen_epoch) <= idle_ttl
        });

        // If still at/above hard cap (e.g. random-JID DDoS), evict oldest non-tombstoned entries first,
        // then oldest entries overall until strictly below cap.
        while self.buckets.len() >= self.config.max_buckets && self.config.max_buckets > 0 {
            let victim = self
                .buckets
                .iter()
                .min_by_key(|(_, b)| {
                    let active_tombstone = b.tombstone_until.map(|u| now_epoch < u).unwrap_or(false);
                    (active_tombstone, b.last_seen_epoch)
                })
                .map(|(k, _)| k.clone());

            match victim {
                Some(k) => {
                    self.buckets.remove(&k);
                }
                None => break,
            }
        }

        before.saturating_sub(self.buckets.len())
    }
}

fn normalize_sender_key(sender_jid: &str) -> String {
    sender_jid.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_burst_allows_3_notifies_once_then_silently_drops() {
        let mut limiter = IngressRateLimiter::default();
        let guest = "628111222333@s.whatsapp.net";

        // 5 messages within 2 seconds: 1..3 Allow, 4 NotifyOnceAndDrop, 5 SilentDrop
        assert_eq!(limiter.check(guest, Authority::Guest, 1_000), ThrottleDecision::Allow);
        assert_eq!(limiter.check(guest, Authority::Guest, 1_000), ThrottleDecision::Allow);
        assert_eq!(limiter.check(guest, Authority::Guest, 1_001), ThrottleDecision::Allow);

        match limiter.check(guest, Authority::Guest, 1_001) {
            ThrottleDecision::NotifyOnceAndDrop { retry_after_secs } => {
                assert!(retry_after_secs >= 5);
            }
            other => panic!("Expected NotifyOnceAndDrop on 4th msg, got {:?}", other),
        }

        match limiter.check(guest, Authority::Guest, 1_002) {
            ThrottleDecision::SilentDrop {
                consecutive_hits,
                tombstoned,
                ..
            } => {
                assert_eq!(consecutive_hits, 2);
                assert!(!tombstoned);
            }
            other => panic!("Expected SilentDrop on 5th msg, got {:?}", other),
        }
    }

    #[test]
    fn repeated_violations_trigger_900s_tombstone_and_recover_after_expiry() {
        let mut limiter = IngressRateLimiter::default();
        let spammer = "628999000111@s.whatsapp.net";

        // Exhaust 3 initial tokens
        for i in 0..3 {
            assert_eq!(
                limiter.check(spammer, Authority::Guest, 2_000 + i),
                ThrottleDecision::Allow
            );
        }

        // Violation 1 -> NotifyOnceAndDrop
        assert!(matches!(
            limiter.check(spammer, Authority::Guest, 2_003),
            ThrottleDecision::NotifyOnceAndDrop { .. }
        ));
        // Violations 2..4 -> SilentDrop (not yet tombstoned)
        for expected_hit in 2..=4 {
            assert!(matches!(
                limiter.check(spammer, Authority::Guest, 2_003),
                ThrottleDecision::SilentDrop {
                    consecutive_hits,
                    tombstoned: false,
                    ..
                } if consecutive_hits == expected_hit
            ));
        }
        // Violation 5 -> Tombstoned for 900 seconds!
        assert_eq!(
            limiter.check(spammer, Authority::Guest, 2_003),
            ThrottleDecision::SilentDrop {
                retry_after_secs: 900,
                consecutive_hits: 5,
                tombstoned: true,
            }
        );

        // Even after normal 20s refill interval (t = 2_200), still tombstoned!
        assert_eq!(
            limiter.check(spammer, Authority::Guest, 2_200),
            ThrottleDecision::SilentDrop {
                retry_after_secs: 703,
                consecutive_hits: 6,
                tombstoned: true,
            }
        );

        // After full 900s isolation (t = 2_903), sender recovers cleanly.
        assert_eq!(
            limiter.check(spammer, Authority::Guest, 2_903),
            ThrottleDecision::Allow
        );
    }

    #[test]
    fn admin_completely_bypasses_throttling() {
        let mut limiter = IngressRateLimiter::default();
        let admin = "6289625345646@s.whatsapp.net";

        for _ in 0..25 {
            assert_eq!(
                limiter.check(admin, Authority::Admin, 3_000),
                ThrottleDecision::Allow
            );
        }
        assert_eq!(limiter.tracked_senders(), 0);
    }

    #[test]
    fn staff_gets_generous_quota_and_refills() {
        let mut limiter = IngressRateLimiter::default();
        let staff = "628555666777@s.whatsapp.net";

        for _ in 0..10 {
            assert_eq!(
                limiter.check(staff, Authority::Staff, 4_000),
                ThrottleDecision::Allow
            );
        }
        assert!(matches!(
            limiter.check(staff, Authority::Staff, 4_000),
            ThrottleDecision::NotifyOnceAndDrop { .. }
        ));
        // After 6 seconds, 1 token refills and resets the cooldown window
        assert_eq!(
            limiter.check(staff, Authority::Staff, 4_006),
            ThrottleDecision::Allow
        );
    }

    #[test]
    fn prune_expired_enforces_bounded_memory_cap() {
        let cfg = RateLimitConfig {
            max_buckets: 5,
            idle_ttl_secs: 60,
            ..RateLimitConfig::default()
        };
        let mut limiter = IngressRateLimiter::new(cfg);

        for i in 0..20 {
            let jid = format!("628000000{:03}@s.whatsapp.net", i);
            let _ = limiter.check(&jid, Authority::Guest, 1_000 + i);
        }
        assert!(
            limiter.tracked_senders() <= 5,
            "Tracked buckets must never exceed max_buckets=5, got {}",
            limiter.tracked_senders()
        );

        // Advance time beyond idle_ttl_secs -> all idle buckets pruned
        let removed = limiter.prune_expired(5_000);
        assert!(removed > 0);
        assert_eq!(limiter.tracked_senders(), 0);
    }
}
