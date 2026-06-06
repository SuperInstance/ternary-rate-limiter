//! # ternary-rate-limiter
//!
//! Rate limiter with ternary feedback for GPU kernel submissions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateSignal { SpeedUp = 1, Normal = 0, Throttle = -1 }

pub struct RateLimiter {
    tokens: f64,
    max_tokens: f64,
    refill_rate: f64, // tokens per tick
    total_allowed: u64,
    total_rejected: u64,
    tick: u64,
}

impl RateLimiter {
    pub fn new(max_tokens: u32, refill_per_tick: f64) -> Self {
        Self { tokens: max_tokens as f64, max_tokens: max_tokens as f64, refill_rate: refill_per_tick,
            total_allowed: 0, total_rejected: 0, tick: 0 }
    }

    pub fn tick(&mut self) {
        self.tick += 1;
        self.tokens = (self.tokens + self.refill_rate).min(self.max_tokens);
    }

    pub fn try_acquire(&mut self, cost: f64) -> bool {
        if self.tokens >= cost {
            self.tokens -= cost;
            self.total_allowed += 1;
            true
        } else {
            self.total_rejected += 1;
            false
        }
    }

    pub fn signal(&self) -> RateSignal {
        let ratio = self.tokens / self.max_tokens;
        if ratio > 0.7 { RateSignal::SpeedUp }
        else if ratio > 0.3 { RateSignal::Normal }
        else { RateSignal::Throttle }
    }

    pub fn rate(&self) -> f64 {
        let total = self.total_allowed + self.total_rejected;
        if total == 0 { return 1.0; }
        self.total_allowed as f64 / total as f64
    }

    pub fn tokens(&self) -> f64 { self.tokens }
    pub fn allowed(&self) -> u64 { self.total_allowed }
    pub fn rejected(&self) -> u64 { self.total_rejected }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acquire() {
        let mut rl = RateLimiter::new(10, 1.0);
        assert!(rl.try_acquire(1.0));
        assert_eq!(rl.allowed(), 1);
    }

    #[test]
    fn test_reject_when_empty() {
        let mut rl = RateLimiter::new(2, 0.0);
        rl.try_acquire(1.0);
        rl.try_acquire(1.0);
        assert!(!rl.try_acquire(1.0));
        assert_eq!(rl.rejected(), 1);
    }

    #[test]
    fn test_refill() {
        let mut rl = RateLimiter::new(5, 2.0);
        rl.try_acquire(5.0); // drain
        rl.tick(); // refill 2
        assert!(rl.try_acquire(1.0)); // should have 2 tokens
    }

    #[test]
    fn test_signal_speedup() {
        let rl = RateLimiter::new(10, 1.0);
        assert_eq!(rl.signal(), RateSignal::SpeedUp); // full bucket
    }

    #[test]
    fn test_signal_throttle() {
        let mut rl = RateLimiter::new(10, 0.0);
        for _ in 0..10 { rl.try_acquire(1.0); }
        assert_eq!(rl.signal(), RateSignal::Throttle);
    }

    #[test]
    fn test_rate() {
        let mut rl = RateLimiter::new(5, 0.0);
        rl.try_acquire(1.0); rl.try_acquire(1.0); rl.try_acquire(1.0);
        rl.try_acquire(1.0); rl.try_acquire(1.0); rl.try_acquire(1.0); // rejected
        assert!((rl.rate() - 0.833).abs() < 0.05);
    }

    #[test]
    fn test_max_cap() {
        let mut rl = RateLimiter::new(5, 100.0);
        rl.tick(); rl.tick();
        assert_eq!(rl.tokens(), 5.0); // capped at max
    }
}
