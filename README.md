# lau-kintsugi-runtime

**The kintsugi principle for software agents: failures make artifacts MORE valuable, not less.**

Named after the Japanese art of repairing broken pottery with gold-lacquer — where the breakage and repair become part of the object's history, making it more beautiful and valuable than before. This crate applies that philosophy to software: every crash, timeout, and test failure is tracked, repaired, and the resulting "golden value" is added to the artifact's worth.

---

## What This Does

This is a runtime library for tracking failures, repairs, and the accumulated knowledge they produce. It provides:

1. **Break types** — 10 structured failure categories (test failures, build errors, panics, conservation violations, deadlocks, timeouts, model errors, agent crashes, circuit breaks, custom).
2. **Repair records** — Each repair tracks what broke, when, who fixed it, how long it took, what was learned, and how much "golden value" the repair added.
3. **Artifacts** — Software components that accumulate value through break-repair cycles. An artifact that has been broken and repaired is *more valuable* than one that never broke.
4. **Policies** — Configurable rules for auto-repair, golden value multipliers, max repairs, decay rates, and minimum values.
5. **Ledger** — Central registry that tracks all artifacts, ranks them by brokenness/value/resilience, computes break-type distributions, and collects lessons learned.

Everything is pure Rust, zero dependencies beyond `serde` + `serde_json`, and fully serializable.

---

## Key Idea

> **Resilience is earned, not inherent.** An artifact that has survived 5 breaks and been repaired each time, learning from each failure, is more trustworthy than one that has never been tested. The golden value of each repair — the knowledge, the fix, the monitoring — compounds over time.

The resilience score formula: `resilience = √(repair_count) × avg(golden_value)`

---

## Install

```toml
[dependencies]
lau-kintsugi-runtime = "0.1"
```

Requires Rust 2021 edition. Dependencies: `serde`, `serde_json`.

---

## Quick Start

```rust
use lau_kintsugi_runtime::*;

fn main() {
    // Create a ledger
    let mut ledger = KintsugiLedger::new();

    // Register an artifact
    let artifact = KintsugiArtifact::new("auth-service", "microservice")
        .with_id("auth")
        .with_original_value(100.0);
    ledger.register_artifact(artifact);

    // Record a break and repair
    let repair = KintsugiRepair::new(
        "r1", "auth",
        BreakType::Timeout { operation: "db_query".into(), limit_ms: 5000 },
        1000, 1200, // break_time, repair_time
        15.0,       // golden value gained
        "added connection pooling",
        vec!["always pool connections under load".into()],
        "agent-resilience",
    );
    ledger.record_repair("auth", repair).unwrap();

    // Check results
    let a = &ledger.artifacts["auth"];
    println!("Value: {} → {}", a.original_value, a.current_value); // 100 → 115
    println!("More valuable? {}", a.is_more_valuable());           // true
    println!("Resilience: {:.2}", a.resilience_score());
    println!("Lessons: {:?}", ledger.lessons_learned());
}
```

---

## API Reference

### `BreakType` — structured failure categories

```rust
pub enum BreakType {
    TestFailure { test_name, error },
    BuildError { file, line, error },
    RuntimePanic { message, backtrace },
    ConservationViolation { expected, actual },
    Deadlock { thread, held_lock },
    Timeout { operation, limit_ms },
    ModelError { provider, code, message },
    AgentCrash { agent_id, last_state },
    CircuitBreak { circuit_id, value, threshold },
    Custom { category, message },
}
```

Methods: `category() → &str` (returns the variant name or custom category).

### `KintsugiRepair` — a single break-repair event

| field | type | description |
|---|---|---|
| `id` | String | unique repair ID |
| `artifact_id` | String | which artifact was repaired |
| `break_type` | BreakType | what kind of failure |
| `break_time` / `repair_time` | u64 | timestamps (ms since epoch) |
| `golden_value` | f64 | value added by this repair |
| `repair_description` | String | what was done |
| `lessons` | Vec\<String\> | knowledge gained |
| `repairer` | String | who/what performed the repair |

Methods: `duration() → u64`, `value_density() → f64` (golden_value / duration).

### `KintsugiArtifact` — a repairable software component

| method | description |
|---|---|
| `new(name, type)` | create with default value 1.0 |
| `with_id(id)` / `with_original_value(v)` | builder pattern |
| `break_and_repair(repair)` | record a break+repair, add golden value |
| `total_golden_value()` | sum of all repair golden values |
| `is_more_valuable()` | current_value > original_value? |
| `resilience_score()` | √(repairs) × avg(golden) |
| `weakest_break()` | repair with longest duration |
| `strongest_repair()` | repair with highest golden value |

### `KintsugiPolicy` — configurable repair rules

| method | description |
|---|---|
| `new()` / `default()` | auto_repair=true, multiplier=1.1 |
| `with_auto_repair(bool)` | enable/disable automatic repairs |
| `with_multiplier(f64)` | golden value scaling factor |
| `with_max_repairs(usize)` | cap repairs per artifact |
| `with_decay_rate(f64)` | value decay over time |
| `with_min_golden_value(f64)` | minimum repair value threshold |
| `is_within_limits(&artifact)` | check if artifact is within policy |

### `KintsugiLedger` — central artifact registry

| method | description |
|---|---|
| `new()` | empty ledger |
| `register_artifact(artifact)` | add an artifact |
| `record_repair(id, repair) → Result` | record a break-repair (returns error if artifact not found or negative value) |
| `most_broken(n)` | top N artifacts by break count |
| `most_valuable(n)` | top N by current value |
| `most_resilient(n)` | top N by resilience score |
| `total_golden_value()` | sum across all artifacts |
| `break_type_distribution()` | HashMap of category → count |
| `lessons_learned()` | all lessons from all repairs |

### `KintsugiError` — error types

| variant | when |
|---|---|
| `ArtifactNotFound(id)` | repair recorded for unknown artifact |
| `AlreadyRepaired(id)` | duplicate repair attempt |
| `NegativeValue(v)` | golden value < 0 |

Implements `std::error::Error` + `Display`.

---

## How It Works

### Data Flow

```
BreakType → KintsugiRepair → KintsugiArtifact → KintsugiLedger
                                   ↓                    ↓
                            current_value +=      ranking, distribution,
                            golden_value          lessons, serialization
```

### Value Accumulation

Each artifact starts with an `original_value`. Every repair adds its `golden_value` to `current_value`:

```
current_value = original_value + Σ(repairs.golden_value)
```

An artifact `is_more_valuable()` when `current_value > original_value`.

### Resilience Score

```
resilience = √(repair_count) × average(golden_values)
```

The square root means resilience grows sublinearly — early repairs contribute most, but the accumulated experience still compounds.

### Policy Enforcement

Policies gate repair eligibility: `is_within_limits()` checks max_repairs_per_artifact. The `auto_repair` flag, `golden_value_multiplier`, `decay_rate`, and `min_golden_value` are available for higher-level runtimes to implement custom logic.

### Serialization

All types implement `Serialize` + `Deserialize` via serde. The entire ledger can be persisted to JSON and restored:

```rust
let json = serde_json::to_string(&ledger).unwrap();
let restored: KintsugiLedger = serde_json::from_str(&json).unwrap();
```

---

## The Math

### Value Model

For an artifact A with original value v₀ and n repairs with golden values g₁, ..., gₙ:

```
V(A) = v₀ + Σᵢ gᵢ
```

The artifact is "kintsugi-enhanced" when V(A) > v₀.

### Resilience Score

```
R(A) = √n × (1/n) Σᵢ gᵢ = Σᵢ gᵢ / √n
```

This balances quantity of repairs against their quality. Few high-value repairs beat many low-value ones.

### Value Density

For a repair taking duration d:

```
ρ(r) = g / d
```

High density = the repair added a lot of value quickly. Low density = slow fix for small gain.

---

## Test Suite

**61 tests** covering all types, edge cases, serialization roundtrips, and integration scenarios:

| area | tests |
|---|---|
| BreakType | categories, serde, equality |
| KintsugiRepair | duration, density, serde |
| KintsugiArtifact | break/repair, value tracking, resilience, weakest/strongest, builder |
| KintsugiPolicy | defaults, builder, limits, serde |
| KintsugiError | display, equality, std::Error |
| KintsugiLedger | register, record, ranking, distribution, lessons, serde |
| Integration | full journey, all break types, multiple artifacts |

Run: `cargo test`

---

## License

MIT
