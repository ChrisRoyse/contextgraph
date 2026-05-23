# futurebuild — ME-JEPA-Code Reference

Source-grounded WHAT-IS reference for AI agents. 8 docs.

- **Source code:** `/home/cabdru/contextgraph/crates/` (22 active workspace crates)
- **Authority:** `docs2/AICodingAgentSuperPrompt.md` (universal doctrine) > `CLAUDE.md` (project addendum) > this suite

---

## ⚠ 2026-05-22 hardening pivot — gate-active

Operator opened **#633 [P0] Manual full-system FSV + hardening gate before new work** after 5-Sherlock parallel forensic audit (`#629` closeout) shipped ~40 P0 closures + ~15 new P0 findings. **#392 / #511 / #405 work suspended** until #633 child queue resolves.

```
#633 manual full-system FSV (children #634-#651, 17 sub-tasks)
 → #655 retire Rust FSV examples → manual SoT (in-progress p0)
 → #656 mejepa-heal-drill conformal-overcoverage repair (in-progress p0)
 → #657 TASK-PY-G-136 per-embedder geometry-label bakeoff (in-progress p0)
 → resume #615/#616 data-substrate work, then #392 trajectory ECE
```

Per #655, Rust `*_fsv.rs` examples are being retired. Closure evidence is now manual operator SoT readback only — read actual bytes/rows/files/services BEFORE and AFTER each trigger, paste both into issue comment, exercise happy path + ≥3 edge classes. Return values, green scripts, prior FSV JSON are supporting evidence only, not the verdict. See `06 §1.9` for the binding contract.

**Doctrine-violation P0s opened by the audit (open, gate-blocking):**

| Issue | Defect |
|---|---|
| #618 / TASK-PY-G-136 | Aggregate OOD score uses flat 5,120-byte packed panel buffer (doctrine §1.5ter violation) |
| #619 / TASK-PY-G-137 | `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter violation) |
| #621 / TASK-PY-G-138 | Hardcoded `gtau_pass_rate: 1.0` published to MCP weekly dashboard |
| #670 | Regenerate or retire invalid forward-cache spaces `e2/e3/e4/e8` |

**#372 aiwonder ZFS root migration CLOSED 2026-05-22.** `/zfs/archive/contextgraph` → `archive/contextgraph` ZFS dataset; `/zfs/hot/contextgraph` → `hotpool/contextgraph` ZFS. Legacy mounted-drive paths (`/mnt/d`, `/mnt/c`) forbidden in active code/FSV. Enforced behaviorally by `scripts/ci/check_no_c_drive_paths.sh` + the #612 → #613 → #614 → #615 → #631 → #632 → #641 → #666 audit chain.

---

## ⚠ Synthetic-vector remediation (2026-05-19/20) — historical context

Between Phase G ramp and 2026-05-19 22:45 UTC, **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of real frozen `E*Instrument` encoders. Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452) document and fix this — all closed 2026-05-19/20.

**Survived:**
- 399,777 AST chunks + 391-shard packed forward cache + 12 active embedders (real Candle CUDA forward, ~24h GPU, SHA-pinned safetensors, fail-closed `"no deterministic vector fallback is permitted"` guards in `crates/context-graph-mejepa-embedders/src/forward.rs`)
- Anything deterministically derived from real cache: #388 cross-embedder tensors, #389 per-slot calibration, #413 Level-0 labels + 491-row accepted registry, #414 skill substrate (128 / 36,773 / 10,579), #404 hierarchy (8 / 157 / 379)
- All structural/invariant work: slot-preserving doctrine, anti-compensation, ambiguity boundary, mistake-loop machinery, runtime ability resolver

**Invalidated:**
- The `0.866667` ship-gate measurement and the `λ_inv ∈ {0, 0.25, 0.5, 1.0}` ablation (every variant measured `0.8666666746139526` — same synthetic input)
- Pre-fix #393 capability-discovery + #398 calibrated reruns (real-input reruns now exist, remain `no_promotion`)
- The "slot-preserving doesn't close the 0.083 gap" claim and "gap is a function of labels, not embedders/storage" claim (both invalid as broad causal claims)

**Post-fix baseline:** `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` — `prediction_oracle_correlation` raw `0.4843192995`, rank-safe `0.6916886568`, rank-lift `0.7942190170`; Brier `0.0923729688`, ECE `0.0240316074`; report sha256 `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499`.

Earlier row-trace `run-20260520T185945Z` preserves #536 metrics + row-level calibration/holdout traces for #549. **The shipped `0.7942` figure was measured before the 5-Sherlock audit; #633 must revalidate it under manual SoT before any new gate-moving work resumes.**

**#392 active/abstain (diagnostic-only):** `205 / 240` active rows, active corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`. Aggregate clears 0.95 but per-cell/window/power gates fail.

For the full evidence chain (#536/#547/#548/#549/#392/#554/#557/#558/#560/#551), see `07_results_and_benchmarks.md §1`.

---

## Read order

| # | Doc | When |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time. What it is, tech stack, entry points, subsystem summary, ambiguity boundary. |
| 2 | [`02_source_map.md`](02_source_map.md) | Crate/file ownership. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. 200+ RocksDB CFs. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling/implementing an MCP tool. 216 tools (69 `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Predictor work: embedders, instruments, panels, heads, TCT, corpus, training, heal, mistake loop. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Proof history. Marks pre-fix evidence INVALID. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Open issues, critical path, post-ship plans. |

Plus: [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract. [`_archived/`](_archived/) — read-only history.

---

## One-line system summary

ME-JEPA-Code is a **binary reality predictor**: for any AI-generated code change it returns `Pass | Fail` against the Docker oracle, with a named failure mode + closest exemplars on Fail (Q3), claim-exists (Q1) and shift-event (Q5) binary predictions as additional grounded surfaces. On top of the binary primitive it surfaces a **butterfly-effect pathway tree** (#416): top-K (3-5) most-probable downstream futures per candidate patch as chains of Q1/Q2/Q5 binary leaves, letting the operator pick which future to realize.

**Ship gate:**
```
prediction_oracle_correlation ≥ 0.95 × 4 rolling windows
  on 300×8 SWE-bench Lite Python corpus
  AND Panel B (#405) cross-correlation ≥ 0.95
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Current:** rank-lift `0.7942190170`. **Gap:** `0.1557809830`. **Gate predicate provisional** until #633 manual revalidation.

---

## Critical-path one-liner

**P0 blocker:** the **#633 hardening gate**. Per pinned context #383, gate-active issue order is evidence-weighted; current evidence is "manual SoT verification is missing across host/runtime/storage/GPU/Docker/MPS, the predictor product path, MCP tools, slot-preserving panels, RocksDB CFs, trainer + mistake loop, CLI + hooks + shift log, build + tests + CI, observability, performance, security, documentation, benchmark honesty, and operational practice."

```
#633 + children #634-#651
  ↓
#655 manual SoT guidance / #656 heal-drill / #657 geometry-label bakeoff
  ├── #661 cross-embedder consensus → #662 promotion → #664 router → #665 prediction heads
  ↓
Doctrine repairs: #618 / #619 / #621 / #670
  ↓
Data-substrate: #615 (in-progress) / #616 / #671 / #672 / #673 / #674
  ↓
Resume #392 trajectory ECE → #405 Panel B → 0.95 ship gate
```

Post-0.95 roadmap stays archived/deferred. Active roadmap stops at 0.95 × 4-window gate + Panel B cross-check. Pathway exploration, skill-linkage expansions, info-physics formalization, creativity engine wait.

**Neuroscience-inspired reserve (filed 2026-05-20, DO NOT START until current queue concludes):** 8 contingency issues (#538-#545) at `priority:p3` with explicit `DO NOT START` markers. Reassess only after post-queue gap analysis identifies need. See `08 §6.9`.
