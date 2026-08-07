# Research Retrospective: ABIv2 Memory Contexts

**Status:** Research complete — PoC archived, upstream engagement closed  
**Upstream reference:** [anza-xyz/svm#25](https://github.com/anza-xyz/svm/issues/25) (closed)

## Summary

Our laboratory built a snapshot-based CPI permission rollback PoC to test whether it could outperform Anza's stateless `abi_v2_prepare_for_instruction()` design.

We found a bug in our own PoC (`snapshot.entries.clear()` wiped earlier rollback records within the same CPI frame), fixed it, and verified the fix with 4.29B fuzz executions (0 violations, 0 panics).

Upstream maintainer @LucasSte clarified that the official `agave-runtime/feat/abiv2` branch uses a stateless architecture immune to this bug class. We closed svm#25 with a correction.

## What Survives

- **CoreState module** (`src/core_state.rs`): Active in Rift L1 and Rift Network.
- **Fuzzing methodology**: Reused for scheduler research (agave#14274).
- **Upstream relationship**: Established credibility with Anza runtime team.

## Lessons

1. Build to break — negative proof is still proof.
2. Fuzz early — the bug was caught in minutes, not weeks.
3. Report transparently — honest closure builds more trust than silent retraction.
4. Separate research from production — this was always labeled PoC.

---

_Copyright 2026 Eugeny (RFT-SIRM). License: Apache 2.0._
