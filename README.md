# agave-abiv2-memory-contexts

[![Research](https://img.shields.io/badge/Research-SVM%20Memory%20Isolation-06b6d4?style=for-the-badge)](https://github.com/RFT-SIRM/UltraCore-RFT)
[![Fuzzing](https://img.shields.io/badge/Fuzzing-4.29B%2B%20Exec%20%7C%200%20Violations-22c55e?style=for-the-badge)](#-fuzzing-results)
[![Upstream](https://img.shields.io/badge/Upstream-svm%2325-3b82f6?style=for-the-badge)](https://github.com/anza-xyz/svm/issues/25)
[![License](https://img.shields.io/badge/License-Apache%202.0-eab308?style=for-the-badge)](LICENSE)

**SVM Memory Isolation Research · CPI Permission Leakage Mitigation · Complementary Runtime**

_Part of the [UltraCore RFT](https://github.com/RFT-SIRM/UltraCore-RFT) execution platform_

* * *

## 🎯 Overview

This repository investigates the correctness boundary of per-CPI-frame writable permission rollback in the Agave Solana Virtual Machine (SVM). It documents a concrete permission-leakage bug, provides a fix, and validates the solution through extended deterministic fuzzing.

```mermaid
flowchart LR
    subgraph SVM["Agave SVM"]
        CPI["CPI Frame"]
        PERM["Account Permissions"]
    end
    subgraph BUG["Bug: Permission Leakage"]
        UPD1["update_account_permissions<br/>region 0: false→true"]
        UPD2["update_account_permissions<br/>region 1: false→true"]
        CLR["snapshot.entries.clear()<br/>← wipes rollback data"]
        POP["pop()"]
        LEAK["region 0 stays Writable<br/>🔴 LEAKED"]
    end
    subgraph FIX["Fix: Per-Frame First-Touch"]
        FT["record on FIRST touch only"]
        RB["rollback all regions<br/>🟢 RESTORED"]
    end
    SVM --> BUG
    BUG --> FIX
```

* * *

## 🔬 Motivation

During CPI execution in Agave SVM, a program can modify the writable permission of an account and — depending on how `update_account_permissions` is called — that change can persist beyond the CPI frame that initiated it.

This repository investigates the correctness boundary of per-frame rollback and documents a concrete bug found and fixed via extended fuzzing.

* * *

## 🐛 Bug Found: Permission Leakage on Multiple Updates Within One Frame

### The Problem

The original implementation cleared the current frame's rollback list on **every** call to `update_account_permissions`:

```rust
let snapshot = self.snapshots.last_mut().unwrap();
snapshot.entries.clear(); // ← wipes earlier rollback data in the same frame
```

If a single CPI frame issues more than one permission update before returning, the second call **erases** the rollback record written by the first. On `pop()`, only the last-touched account is restored; every earlier account retains its modified permission permanently.

### Reproducing Scenario

```rust
contexts.push_placeholder();                          // open CPI frame
contexts.update_account_permissions(&[(0, true)]);    // region 0: false → true
contexts.update_account_permissions(&[(1, true)]);    // region 1: false → true
                                                      // BUG: region 0 rollback is gone
contexts.pop();
// expected: region 0 = ReadOnly, region 1 = ReadOnly
// actual:   region 0 = Writable  ← leaked
```

### The Fix

Record each account's pre-frame value **only on its first touch** within the frame, never overwriting it on subsequent calls.

```mermaid
flowchart TB
    subgraph BEFORE["❌ Before (Bug)"]
        B1["touch region 0 → record rollback"]
        B2["touch region 1 → clear() → record rollback"]
        B3["pop() → restore region 1 ONLY"]
        B4["region 0: leaked Writable 🔴"]
    end
    subgraph AFTER["✅ After (Fix)"]
        A1["touch region 0 → record rollback (first touch)"]
        A2["touch region 1 → record rollback (first touch)"]
        A3["pop() → restore region 0 AND region 1"]
        A4["all regions: restored ReadOnly 🟢"]
    end
    BEFORE --> AFTER
```

See `src/memory_contexts.rs` and the regression test `multiple_permission_updates_in_one_frame_all_roll_back`.

* * *

## ⚖️ CoreState: Supply/Burn Invariant Module

`src/core_state.rs` (used by the fuzz harness only, not exported from `lib.rs`) is an independent accounting component enforcing:

```
TOTAL_SUPPLY == TOTAL_MINTED - TOTAL_BURNED
TOTAL_SUPPLY == TOTAL_BASE_SUM + (GLOBAL_FIELD × P)
```

### Two Bugs Found and Fixed via Fuzzing

| # | Bug | Impact | Fix |
|---|-----|--------|-----|
| 1 | `unregister_participant` and `apply_transfer` adjusted `total_base_sum` by an arbitrary delta after already rebalancing it, without reflecting that delta back into `total_supply` / `total_minted` | Breaks invariant 2 | Compute mint/burn deltas atomically; derive `total_supply` and `total_base_sum` from the validated result in a single step |
| 2 | Burn accounting used `saturating_sub`, silently masking the case where `burned > minted` instead of rejecting it | Corrupts invariant 1 | Replace with `checked_sub` + explicit error on underflow |

* * *

## ✅ Fuzzing Results

Extended CI fuzz run (`cargo fuzz run fuzz_target_1`, libFuzzer + `arbitrary`):

```mermaid
flowchart LR
    subgraph INPUT["Fuzz Input"]
        R["register_participant"]
        U["unregister_participant"]
        T["apply_transfer"]
        D["redistribute_amount"]
        N["apply_neg_entropy_tick"]
    end
    subgraph HARNESS["Fuzz Harness"]
        MUT["≤ 8 mutations / exec"]
        INV["assert both invariants"]
    end
    subgraph RESULT["Result"]
        OK["4.29B+ exec<br/>0 violations<br/>0 panics 🟢"]
    end
    INPUT --> HARNESS --> RESULT
```

| Metric | Value |
|--------|-------|
| **Duration** | 5 h 55 m (21 300 s) |
| **Total executions** | 4 294 967 296+ |
| **Execution speed** | ~421 000 exec/s (GitHub Actions vCPU) |
| **RSS (stable)** | ~505 MB |
| **Invariant violations** | **0** |
| **Panics** | **0** |

Coverage stabilised at `cov: 53 ft: 166` by the first billion iterations, indicating the harness exhausted the reachable state space for this input size range before the run ended.

> **Note:** These numbers describe the fuzz harness on an isolated in-memory struct. They are **not** SVM transaction throughput figures and should not be compared to validator TPS benchmarks.

* * *

## 📁 Repository Layout

```
src/
  lib.rs                    — public API: memory_contexts, scheduler, shared_memory_protocol
  memory_contexts.rs        — per-frame writable permission tracking (bug + fix documented above)
  scheduler.rs              — conflict-aware transaction scheduling research
  shared_memory_protocol.rs — TPU/worker shared-memory message layouts
  core_state.rs             — standalone supply/mint/burn accounting (fuzz-only)
tests/
  integration_tests.rs      — integration and regression tests
fuzz/
  fuzz_targets/fuzz_target_1.rs — stateful invariant fuzzer
.github/workflows/          — CI: fast checks on push, 5h 55m fuzz on schedule
```

* * *

## 🚀 Running Tests and Fuzzing

```bash
# Unit + integration tests
cargo test

# 5-minute local fuzz run
cargo +nightly fuzz run fuzz_target_1 -- -max_total_time=300

# Full scheduled run (matches CI)
cargo +nightly fuzz run fuzz_target_1 -- -max_total_time=21300
```

* * *

## 🔮 Status and Next Steps

This is a **research repository**, not a production patch. The immediate goal is to:

1. **Integrate** `MemoryContexts` into a fork of `anza-xyz/svm` (transaction-context crate) to measure per-CPI-frame rollback cost against their existing benchmarks.
2. **Open a focused RFC / Draft PR** against `solana-transaction-context` with `cargo bench` before/after numbers and the fuzz corpus as evidence.

Architectural context and theoretical foundations: [UltraCore RFT](https://github.com/RFT-SIRM/UltraCore-RFT)

Upstream issue: [anza-xyz/svm#25](https://github.com/anza-xyz/svm/issues/25)

* * *

## 🔗 Ecosystem Context

| Repository | Role | Relation |
|------------|------|----------|
| [UltraCore-RFT](https://github.com/RFT-SIRM/UltraCore-RFT) | Central laboratory & documentation | Parent organization |
| [Rift-L1-Blockchain](https://github.com/RFT-SIRM/Rift-L1-Blockchain) | Standalone Rust validator core | Same invariants, different runtime |
| [Rift-Network](https://github.com/RFT-SIRM/Rift-Network) | Solana on-chain protocol | Same invariants, Solana runtime |
| [agave-rift-scheduler](https://github.com/RFT-SIRM/agave-rift-scheduler) | Conflict-aware scheduling | Complementary runtime research |

* * *

## 📋 License

[![License](https://img.shields.io/badge/License-Apache%202.0-eab308?style=for-the-badge)](LICENSE)

 **[Apache License 2.0](LICENSE)** 

* * *

**Built in Rust · Verified by Mathematics · Zero Compromises**

_Part of the UltraCore RFT Execution Platform · © 2026 Eugeny (RFT-SIRM)_
