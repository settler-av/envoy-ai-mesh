//! Rate Limiter Module
//!
//! Provides per-agent rate limiting using Wasm shared data.
//! Note: In Wasm, shared data is scoped to the Envoy worker,
//! so this provides approximate rate limiting.

use std::collections::HashMap;

/// Rate limiting configuration
#[derive(Clone, Debug)]
pub struct RateLimits {
    /// Maximum requests per minute
    pub requests_per_minute: u32,
    /// Maximum tokens per minute
    pub tokens_per_minute: u32,
    /// Maximum concurrent requests (not enforced in Wasm)
    pub concurrent_requests: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self {
            requests_per_minute: 100,
            tokens_per_minute: 100_000,
            concurrent_requests: 10,
        }
    }
}

/// Rate limiter state
#[derive(Clone, Debug, Default)]
struct RateState {
    /// Requests in current window
    request_count: u32,
    /// Tokens in current window
    token_count: u32,
    /// Window start timestamp (seconds)
    window_start: u64,
}

/// Rate limiter
pub struct RateLimiter {
    limits: RateLimits,
    /// Per-agent state (simplified in-memory for Wasm)
    state: HashMap<String, RateState>,
    /// Window duration in seconds
    window_seconds: u64,
}

impl RateLimiter {
    /// Create a new rate limiter with default limits
    pub fn new() -> Self {
        Self::with_limits(RateLimits::default())
    }

    /// Create with custom limits
    pub fn with_limits(limits: RateLimits) -> Self {
        Self {
            limits,
            state: HashMap::new(),
            window_seconds: 60, // 1 minute window
        }
    }

    /// Check if a request should be allowed
    ///
    /// Note: `current_time` should be provided by Envoy's `get_current_time_nanoseconds()`
    pub fn check_request(&mut self, agent_id: &str, current_time_secs: u64) -> RateDecision {
        let requests_per_minute = self.limits.requests_per_minute;
        let window_seconds = self.window_seconds;
        let state = self.get_or_create_state(agent_id, current_time_secs);

        // Check if we've exceeded request limit
        if state.request_count >= requests_per_minute {
            return RateDecision::RateLimited(RateLimitInfo {
                reason: "requests_per_minute exceeded".to_string(),
                limit: requests_per_minute,
                current: state.request_count,
                retry_after_secs: window_seconds
                    - (current_time_secs - state.window_start).min(window_seconds),
            });
        }

        // Increment request count
        state.request_count += 1;

        RateDecision::Allow
    }

    /// Record token usage
    pub fn record_tokens(
        &mut self,
        agent_id: &str,
        tokens: u32,
        current_time_secs: u64,
    ) -> RateDecision {
        let tokens_per_minute = self.limits.tokens_per_minute;
        let window_seconds = self.window_seconds;
        let state = self.get_or_create_state(agent_id, current_time_secs);

        // Check if adding tokens would exceed limit
        if state.token_count + tokens > tokens_per_minute {
            return RateDecision::RateLimited(RateLimitInfo {
                reason: "tokens_per_minute exceeded".to_string(),
                limit: tokens_per_minute,
                current: state.token_count,
                retry_after_secs: window_seconds
                    - (current_time_secs - state.window_start).min(window_seconds),
            });
        }

        // Record tokens
        state.token_count += tokens;

        RateDecision::Allow
    }

    /// Get current state for an agent
    pub fn get_state(&self, agent_id: &str) -> Option<RateStateInfo> {
        self.state.get(agent_id).map(|s| RateStateInfo {
            request_count: s.request_count,
            token_count: s.token_count,
            window_start: s.window_start,
        })
    }

    /// Reset state for an agent
    pub fn reset(&mut self, agent_id: &str) {
        self.state.remove(agent_id);
    }

    /// Reset all state
    pub fn reset_all(&mut self) {
        self.state.clear();
    }

    fn get_or_create_state(&mut self, agent_id: &str, current_time_secs: u64) -> &mut RateState {
        let window_seconds = self.window_seconds;

        self.state
            .entry(agent_id.to_string())
            .and_modify(|s| {
                // Check if window has expired
                if current_time_secs - s.window_start >= window_seconds {
                    // Reset for new window
                    s.request_count = 0;
                    s.token_count = 0;
                    s.window_start = current_time_secs;
                }
            })
            .or_insert_with(|| RateState {
                request_count: 0,
                token_count: 0,
                window_start: current_time_secs,
            })
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of rate limit check
#[derive(Debug, Clone)]
pub enum RateDecision {
    /// Request is allowed
    Allow,
    /// Request is rate limited
    RateLimited(RateLimitInfo),
}

impl RateDecision {
    /// Check if rate limited
    pub fn is_limited(&self) -> bool {
        matches!(self, RateDecision::RateLimited(_))
    }

    /// Get rate limit info if limited
    pub fn limit_info(&self) -> Option<&RateLimitInfo> {
        match self {
            RateDecision::RateLimited(info) => Some(info),
            RateDecision::Allow => None,
        }
    }
}

/// Information about rate limiting
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// Reason for rate limiting
    pub reason: String,
    /// The limit that was exceeded
    pub limit: u32,
    /// Current count
    pub current: u32,
    /// Seconds until rate limit resets
    pub retry_after_secs: u64,
}

/// Public view of rate state
#[derive(Debug, Clone)]
pub struct RateStateInfo {
    pub request_count: u32,
    pub token_count: u32,
    pub window_start: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_under_limit() {
        let mut limiter = RateLimiter::with_limits(RateLimits {
            requests_per_minute: 10,
            ..Default::default()
        });

        let result = limiter.check_request("agent-1", 1000);
        assert!(matches!(result, RateDecision::Allow));
    }

    #[test]
    fn test_rate_limit_exceeded() {
        let mut limiter = RateLimiter::with_limits(RateLimits {
            requests_per_minute: 2,
            ..Default::default()
        });

        // First two requests should pass
        assert!(matches!(limiter.check_request("agent-1", 1000), RateDecision::Allow));
        assert!(matches!(limiter.check_request("agent-1", 1001), RateDecision::Allow));

        // Third should be limited
        let result = limiter.check_request("agent-1", 1002);
        assert!(result.is_limited());
    }

    #[test]
    fn test_window_reset() {
        let mut limiter = RateLimiter::with_limits(RateLimits {
            requests_per_minute: 1,
            ..Default::default()
        });

        // First request passes
        assert!(matches!(limiter.check_request("agent-1", 1000), RateDecision::Allow));

        // Second is limited
        assert!(limiter.check_request("agent-1", 1001).is_limited());

        // After window expires (60s), should pass again
        assert!(matches!(limiter.check_request("agent-1", 1061), RateDecision::Allow));
    }

    #[test]
    fn test_token_limit() {
        let mut limiter = RateLimiter::with_limits(RateLimits {
            tokens_per_minute: 100,
            ..Default::default()
        });

        // Record 50 tokens - should pass
        assert!(matches!(
            limiter.record_tokens("agent-1", 50, 1000),
            RateDecision::Allow
        ));

        // Record another 60 tokens - should be limited
        let result = limiter.record_tokens("agent-1", 60, 1001);
        assert!(result.is_limited());
    }

    #[test]
    fn test_per_agent_isolation() {
        let mut limiter = RateLimiter::with_limits(RateLimits {
            requests_per_minute: 1,
            ..Default::default()
        });

        // Agent 1 makes a request
        assert!(matches!(limiter.check_request("agent-1", 1000), RateDecision::Allow));
        assert!(limiter.check_request("agent-1", 1001).is_limited());

        // Agent 2 should still be allowed
        assert!(matches!(limiter.check_request("agent-2", 1001), RateDecision::Allow));
    }
}
