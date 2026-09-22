use std::sync::Mutex;
use std::time::Instant;

use papokin_config::PacketLimiterConfig;

#[derive(Debug)]
struct LimiterState {
    tokens: f64,
    last_update: Instant,
}

/// 基于令牌桶的客户端传入数据包速率限制器。
#[derive(Debug)]
pub struct PacketRateLimiter {
    enabled: bool,
    max_rate: f64,
    burst_capacity: f64,
    state: Mutex<LimiterState>,
}

impl PacketRateLimiter {
    #[must_use]
    pub fn new(enabled: bool, max_rate: f64, burst_capacity: f64) -> Self {
        Self {
            enabled,
            max_rate,
            burst_capacity,
            state: Mutex::new(LimiterState {
                tokens: burst_capacity,
                last_update: Instant::now(),
            }),
        }
    }

    #[must_use]
    pub fn from_config(config: &PacketLimiterConfig) -> Self {
        Self::new(
            config.enabled,
            config.max_packet_rate,
            config.burst_capacity,
        )
    }

    /// 检查传入的数据包在速率限制下是否被允许。
    ///
    ///若数据包在限制范围内则返回 `true`，若超出速率限制则返回 `false`。
    #[must_use]
    pub fn check_packet(&self) -> bool {
        if !self.enabled || self.max_rate <= 0.0 {
            return true;
        }

        let now = Instant::now();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.last_update = now;

        state.tokens = (state.tokens + elapsed * self.max_rate).min(self.burst_capacity);

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub const fn max_rate(&self) -> f64 {
        self.max_rate
    }

    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limiter_allows_packets_within_capacity() {
        let limiter = PacketRateLimiter::new(true, 10.0, 5.0);
        for _ in 0..5 {
            assert!(limiter.check_packet());
        }
        // 爆发耗尽
        assert!(!limiter.check_packet());
    }

    #[test]
    fn limiter_disabled() {
        let limiter = PacketRateLimiter::new(false, 10.0, 1.0);
        for _ in 0..100 {
            assert!(limiter.check_packet());
        }
    }

    #[test]
    fn limiter_zero_rate() {
        let limiter = PacketRateLimiter::new(true, 0.0, 1.0);
        for _ in 0..100 {
            assert!(limiter.check_packet());
        }
    }
}
