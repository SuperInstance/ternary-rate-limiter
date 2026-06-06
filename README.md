# ternary-rate-limiter

Rate limiter for GPU kernel submissions with ternary feedback. Too fast = -1 (throttle), normal = 0, room available = +1 (speed up). Token bucket with ternary signals.

## Why This Matters

# ternary-rate-limiter
Rate limiter with ternary feedback for GPU kernel submissions.

## The Five-Layer Stack

This crate is part of the **Oxide Stack** — a distributed GPU runtime built on five layers:

```
┌─────────────────┐
│  cudaclaw        │  Persistent GPU kernels, warp consensus, SmartCRDT
├─────────────────┤
│  cuda-oxide      │  Flux → MIR → Pliron → NVVM → PTX compiler
├─────────────────┤
│  flux-core       │  Bytecode VM + A2A agent protocol
├─────────────────┤
│  pincher         │  "Vector DB as runtime, LLM as compiler"
├─────────────────┤
│  open-parallel   │  Async runtime (tokio fork)
└─────────────────┘
```

The key insight: **ternary values {-1, 0, +1} map directly to GPU compute**. They pack 16× denser than FP32, enable XNOR+popcount matmul, and conservation laws become compile-time checks.

## Design

Every value in this crate follows **ternary algebra** (Z₃):

| Value | Meaning | GPU Analog |
|-------|---------|------------|
| +1 | Positive / Active / Healthy | Warp vote yes |
| 0 | Neutral / Pending / Balanced | Warp vote abstain |
| -1 | Negative / Failed / Overloaded | Warp vote no |

This isn't arbitrary — ternary is the natural encoding for:
1. **BitNet b1.58** (Microsoft) — ternary LLMs at 60% less power
2. **GPU warp voting** — hardware ballot returns ternary consensus
3. **Conservation laws** — {-1, 0, +1} preserves quantity

## Key Types

```rust
pub enum RateSignal
pub struct RateLimiter
pub fn new
pub fn tick
pub fn try_acquire
pub fn signal
pub fn rate
pub fn tokens
pub fn allowed
pub fn rejected
```

## Usage

```toml
[dependencies]
ternary-rate-limiter = "0.1.0"
```

```rust
use ternary_rate_limiter::*;
// See src/lib.rs tests for complete working examples
```

## Testing

```bash
git clone https://github.com/SuperInstance/ternary-rate-limiter.git
cd ternary-rate-limiter
cargo test    # 7 tests
```

## Stats

| Metric | Value |
|--------|-------|
| Tests | 7 |
| Lines of Rust | 113 |
| Public API | 10 items |

## License

Apache-2.0
