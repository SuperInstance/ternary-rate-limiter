# ternary-rate-limiter

Token-bucket rate limiter with ternary feedback. Too fast → throttle (-1). Normal → steady (0). Room available → speed up (+1).

Most rate limiters give you a boolean: allowed or rejected. This one gives you a *signal*. After each request, you can ask the limiter how it's feeling: do I have headroom to send more? Am I in the danger zone? Or should I slow down? The ternary feedback signal tells upstream systems how to self-regulate without waiting for hard rejections.

## Why this exists

In a GPU cluster, kernel submissions arrive in bursts. A hard rate limit creates a cliff: everything's fine until suddenly it's not, and requests start failing. Ternary feedback creates a gradient instead. You see the throttle signal *before* you hit the limit. Your system can ease off the gas instead of slamming the brakes.

The ternary signal maps directly to load-shedding strategies:

| Signal | Value | What to do |
|--------|-------|------------|
| `SpeedUp` | +1 | Bucket > 70% full — send more work |
| `Normal` | 0 | Bucket 30-70% — maintain current rate |
| `Throttle` | -1 | Bucket < 30% — back off |

## The key insight

Rate limiting isn't just about rejection—it's about *communication*. A traditional rate limiter speaks binary: yes or no. A ternary rate limiter speaks in gradients. The upstream system doesn't have to wait for failures to adapt. It can read the signal and proactively adjust.

This turns rate limiting from a gate into a *feedback loop*. The limiter doesn't just protect downstream resources—it teaches upstream systems how to behave.

## Quick start

```rust
use ternary_rate_limiter::*;

// Create a limiter: max 100 tokens, refills 10 per tick
let mut rl = RateLimiter::new(100, 10.0);

// Submit work
assert!(rl.try_acquire(5.0));   // 95 tokens remaining
assert!(rl.try_acquire(50.0));  // 45 tokens remaining

// Check the signal
assert_eq!(rl.signal(), RateSignal::Normal);  // 45% full

// Refill over time
rl.tick();  // adds 10 tokens → 55

// Check stats
println!("Allowed: {}", rl.allowed());    // 2
println!("Rejected: {}", rl.rejected());  // 0
println!("Rate: {:.1}%", rl.rate() * 100.0);  // 100%
```

## API reference

### RateLimiter

```rust
// Constructor: max token capacity, refill rate per tick
let mut rl = RateLimiter::new(max_tokens, refill_per_tick);

// Core operations
rl.tick();                        // refill tokens (capped at max)
rl.try_acquire(cost);             // → bool (true = allowed)

// Ternary feedback
rl.signal();                      // → RateSignal {SpeedUp, Normal, Throttle}

// Statistics
rl.rate();                        // → f64, fraction of requests allowed (0..1)
rl.tokens();                      // → f64, current token count
rl.allowed();                     // → u64, total allowed requests
rl.rejected();                    // → u64, total rejected requests
```

### RateSignal

```rust
pub enum RateSignal {
    SpeedUp = 1,    // bucket > 70% full
    Normal = 0,     // bucket 30-70%
    Throttle = -1,  // bucket < 30%
}
```

## Signal thresholds

The three zones are defined by token bucket fill ratio:

```
0%                          30%         70%                          100%
├─────────── THROTTLE ──────┼── NORMAL ──┼──────── SPEEDUP ──────────┤
│  Bucket nearly empty.     │  Steady.   │  Bucket has headroom.    │
│  Back off submissions.    │  Keep it.  │  Send more work.         │
```

## Real-world example: Adaptive GPU submission

```rust
use ternary_rate_limiter::*;

struct KernelSubmitter {
    limiter: RateLimiter,
    pending: Vec<f64>,  // pending kernel costs
}

impl KernelSubmitter {
    fn new() -> Self {
        Self {
            limiter: RateLimiter::new(100, 5.0),  // 100 tokens, 5/tick
            pending: vec![],
        }
    }

    fn submit(&mut self, cost: f64) -> bool {
        if self.limiter.try_acquire(cost) {
            // Actually submit the kernel
            true
        } else {
            // Queue for later
            self.pending.push(cost);
            false
        }
    }

    fn tick(&mut self) {
        self.limiter.tick();

        // Use the ternary signal to adapt behavior
        match self.limiter.signal() {
            RateSignal::SpeedUp => {
                // Bucket is full — drain the pending queue aggressively
                self.pending.retain(|&cost| !self.limiter.try_acquire(cost));
            }
            RateSignal::Normal => {
                // Moderate drain — submit one pending if possible
                if let Some(cost) = self.pending.first() {
                    if self.limiter.try_acquire(*cost) {
                        self.pending.remove(0);
                    }
                }
            }
            RateSignal::Throttle => {
                // Bucket nearly empty — don't submit anything
            }
        }
    }

    fn health_report(&self) -> String {
        format!(
            "Tokens: {:.0}/{} | Signal: {:?} | Rate: {:.0}% | Pending: {}",
            self.limiter.tokens(),
            100,
            self.limiter.signal(),
            self.limiter.rate() * 100.0,
            self.pending.len(),
        )
    }
}
```

## Architecture

The implementation is a classic token bucket:

```
                  refill_rate per tick
                       │
                       ▼
    ┌──────────────────────────────┐
    │      Token Bucket            │
    │  tokens: f64                 │◄── try_acquire(cost)
    │  max: f64                    │        │
    │                              │        ├─ tokens >= cost → allowed
    │  ┌──────────────────────┐    │        └─ tokens < cost  → rejected
    │  │    Fill ratio        │    │
    │  │  tokens / max_tokens │    │
    │  └──────┬───────────────┘    │
    │         │                    │
    │    ratio > 0.7 → SpeedUp    │
    │    ratio > 0.3 → Normal     │
    │    else       → Throttle    │
    └──────────────────────────────┘
```

Tokens are `f64` for fractional costs. This means a request can cost 0.5 tokens (half-weight) or 3.7 tokens. The bucket refills by `refill_rate` per `tick()`, capped at `max_tokens`.

## Ecosystem connections

- **ternary-gauge** — gauge the rate limiter's signal stream to detect load patterns over time (is it oscillating between SpeedUp and Throttle? stuck at Throttle?)
- **ternary-version** — version the rate limiter's configuration across distributed nodes
- **ternary-paxos** — use consensus to agree on cluster-wide rate limit settings

## Performance

| Operation | Complexity |
|-----------|-----------|
| `tick` | O(1) — one addition and one min |
| `try_acquire` | O(1) — one comparison and one subtraction |
| `signal` | O(1) — one division and two comparisons |
| `rate` | O(1) — one division |

No allocations in the hot path. The struct is 64 bytes (3 f64s + 2 u64s + 1 u64 for tick counter).

## Stats

| Metric | Value |
|--------|-------|
| Tests | 7 |
| Lines of code | 113 |
| Public API surface | 10 items |
| License | Apache-2.0 |
| Unsafe | 0 |

## Installation

```toml
[dependencies]
ternary-rate-limiter = "0.1.0"
```

## License

Apache-2.0
