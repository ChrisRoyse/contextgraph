---
namespace: discoveries
created: 2026-05-10T17:45:00-05:00
updated: 2026-05-10T17:45:00-05:00
status: active
tags: mejepa, phase5, self-healing, fsv
---

# Phase 5 Self-Healing Root Cause

Phase 5 was blocked by a disabled self-healing surface rather than a narrow runtime bug: `MeJepaCompiler::observe` had been a fail-closed `FeatureDisabled("self-healing")` endpoint, so drift detection, EWC, promotion, LoRA refresh, integrity checks, and FSV drills could not be exercised through a real control path.

During FSV readback, two source-of-truth bugs were also found and fixed:

- EWC memory capped snapshots at 5, but `CF_MEJEPA_FISHER_SNAPSHOTS` retained stale rows. The store now replaces the persisted snapshot set and readback-counts it, so RocksDB matches the in-memory retained set.
- Distillation telemetry was sharing `CF_MEJEPA_HEAL_REPORTS` with promotion reports. A dedicated `CF_MEJEPA_DISTILL_STEPS` column family prevents mixed bincode schemas and keeps promotion status decodable.

The learning formula in `context-graph-mejepa-train` now uses the intended multiplicative signal `L_step = delta_p * delta_k * delta_omega * delta_xi`, not the prior weighted additive approximation.
