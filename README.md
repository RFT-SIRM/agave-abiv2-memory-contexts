# agave-abiv2-memory-contexts

Experimental research module exploring safer per-CPI-frame writable permission
isolation for the Agave SVM ABIv2 memory context layer.

Developed under the [UltraCore RFT Lab](https://github.com/RFT-SIRM/UltraCore-RFT)
research framework.

---

## Motivation

During CPI execution in Agave SVM, a program can modify the writable permission of an
account and — depending on how `update_account_permissions` is called — that change can
persist beyond the CPI frame that initiated it. This repository investigates the
correctness boundary of per-frame rollback and documents a concrete bug found and fixed
via extended fuzzing.

---

## Bug found: permission leakage on multiple updates within one frame

The original implementation cleared the current frame's rollback list on **every** call
to `update_account_permissions`:

```rust
let snapshot = self.snapshots.last_mut().unwrap();
snapshot.entries.clear(); // ← wipes earlier rollback data in the same frame
```

If a single CPI frame issues more than one permission update before returning, the
second call erases the rollback record written by the first. On `pop()`, only the
last-touched account is restored; every earlier account retains its modified permission
permanently.

**Reproducing scenario:**

```rust
contexts.push_placeholder();           // open CPI frame
contexts.update_account_permissions(&[(0, true)]);   // region 0: false → true
contexts.update_account_permissions(&[(1, true)]);   // region 1: false → true
                                                      // BUG: region 0 rollback is gone
contexts.pop();
// expected: region 0 = ReadOnly, region 1 = ReadOnly
// actual:   region 0 = Writable  ← leaked
```

**Fix:** record each account's pre-frame value only on its first touch within the
frame, never overwriting it on subsequent calls. See `src/memory_contexts.rs` and the
regression test `multiple_permission_updates_in_one_frame_all_roll_back`.

---

## CoreState: supply/burn invariant module

`src/core_state.rs` (used by the fuzz harness only, not exported from `lib.rs`) is an
independent accounting component enforcing:

```
TOTAL_SUPPLY == TOTAL_MINTED - TOTAL_BURNED
TOTAL_SUPPLY == TOTAL_BASE_SUM + (GLOBAL_FIELD × P)
```

Two bugs were found and fixed via fuzzing:

1. `unregister_participant` and `apply_transfer` adjusted `total_base_sum` by an
   arbitrary delta *after* already rebalancing it, without reflecting that delta back
   into `total_supply` / `total_minted`, breaking the second invariant.
2. Burn accounting used `saturating_sub`, silently masking the case where burned >
   minted instead of rejecting it, corrupting the first invariant.

Both are fixed by computing mint/burn deltas atomically and deriving `total_supply` and
`total_base_sum` from the validated result in a single step.

---

## Fuzzing results

Extended CI fuzz run (`cargo fuzz run fuzz_target_1`, libFuzzer + `arbitrary`):

| Metric | Value |
|---|---|
| Duration | 5 h 55 m (21 300 s) |
| Total executions | 4 294 967 296+ |
| Execution speed | ~421 000 exec/s (GitHub Actions vCPU) |
| RSS (stable) | ~505 MB |
| Invariant violations | **0** |
| Panics | **0** |

Each execution applies up to 8 randomised state mutations
(`register_participant`, `unregister_participant`, `apply_transfer`,
`redistribute_amount`, `apply_neg_entropy_tick`) and asserts both invariants after
every step.

Coverage stabilised at `cov: 53 ft: 166` by the first billion iterations, indicating
the harness exhausted the reachable state space for this input size range before the
run ended.

> **Note on the figures above.** These numbers describe the fuzz harness on an isolated
> in-memory struct. They are not SVM transaction throughput figures and should not be
> compared to validator TPS benchmarks.

---

## Repository layout

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

---

## Running tests and fuzzing

```bash
# unit + integration tests
cargo test

# 5-minute local fuzz run
cargo +nightly fuzz run fuzz_target_1 -- -max_total_time=300

# full scheduled run (matches CI)
cargo +nightly fuzz run fuzz_target_1 -- -max_total_time=21300
```

---

## Status and next steps

This is a research repository, not a production patch. The immediate goal is to:

1. Integrate `MemoryContexts` into a fork of
   [`anza-xyz/svm`](https://github.com/anza-xyz/svm) (`transaction-context` crate) to
   measure per-CPI-frame rollback cost against their existing benchmarks.
2. Open a focused RFC / Draft PR against `solana-transaction-context` with `cargo bench`
   before/after numbers and the fuzz corpus as evidence.

Architectural context and theoretical foundations:
[UltraCore RFT — Development Strategy](https://github.com/RFT-SIRM/UltraCore-RFT/blob/main/RFT_DEVELOPMENT_STRATEGY.md)

---

## License

Apache-2.0
