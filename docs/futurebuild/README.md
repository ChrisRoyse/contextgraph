# futurebuild — ME-JEPA-Code Reference

Source-grounded WHAT-IS reference for AI agents. 8 docs.

- **Source code:** `/home/cabdru/contextgraph/crates/` (22 active workspace crates)
- **Authority:** `docs2/AICodingAgentSuperPrompt.md` (universal doctrine) > `CLAUDE.md` (project addendum) > this suite

---

## ⚠ 2026-05-23 Sherlock audit — measurement decomposed; critical path replaced

3-Sherlock parallel forensic audit (reports `docs/SHERLOCK_{PREDICTOR,SUBSTRATE,INFRA}_FINDINGS_2026-05-22.md`) decomposed the `0.7942190170` rank-lift measurement into three structural defects:

| Layer | Defect | Issue |
|---|---|---|
| Predictor head | Public Rust `Trainer` hard-coded `DIAGNOSTIC_CERTIFICATE_ONLY_TRAINING_MODE`; latest aiwonder FSV loaded an out-of-band task-py-g-127 checkpoint (`optimizer_steps=64`, `weight_delta_observed=true`), but that path is not countable through the trainer contract and compiler construction must fail closed without an explicit `trained_checkpoint_manifest_path` | #683 |
| Substrate | Legacy v1 cache had `e2/e3/e4` collapsed and `e8 ≡ e1`. #704/#714 now retire `e2/e3/e4` for static AST corpora; #715 owns the v2 9-slot static cache rebuild, gated by #709 distinctness proof so post-`ff4d4c03` `e8` is not an `e1` alias. | #704, #714, #709, #715 |
| Gate metric | Rank-lift = category-local RBF exemplar similarity over calibration split; lifts raw `0.4843 → 0.7942` (+0.31) without Brier/ECE improvement | #695 |

Honest gap = `0.95 − 0.4843 = 0.4657`, not `0.1558`. The 2026-05-23 audit filed 29 issues (#683-#711), with post-decision static-cache follow-ups #714/#715. New pinned context: **#712**.

**New critical path (supersedes prior `08 §3.0`):**

| Rank | Issue | Action |
|---|---|---|
| 1 | #683 | Unify real training/checkpoint export with the trainer contract; require production `trained_checkpoint_manifest_path` fail-closed |
| 2 | #709 + #715 | Add the forward-cache distinctness gate, then rebuild a v2 static AST cache with 9 active slots (`e1,e6,e7,e8,e9,e10,e12,e13,e14`); `e2/e3/e4` stay retired for static AST by #704/#714 |
| 3 | #615 | delete_test_call Docker oracle batch over 1,131 candidates |
| 4 | #618 + #619 | Per-slot OOD; slot-preserving panel projections |
| 5 | #695 | Replace rank-lift with raw Pearson + isotonic/Platt |
| 6 | #620/#621/#622/#684/#685/#686/#687/#688/#689/#690/#692/#696/#699/#705/#706 | Eliminate hardcoded/no-op telemetry, pseudo-cert confidence, dead public training surfaces, and non-countable Q5/OOD claims |
| Done | #704/#714/#670/#691/#707 | Static temporal retirement, invalid-space policy, hook fail-visible behavior, strict OOD fail-closed behavior |
| 7 | #633 + #655 + #656 + #657 and children | Manual full-system FSV/hardening gate, auto-FSV retirement, heal-drill truthfulness, and geometry-label bakeoff under clean 9-slot policy |
| 8 | #392 / #511 / #405 / #379 | **Blocked behind active ranks 1-7** |

## ⚠ 2026-05-22 hardening pivot — still active

Operator opened **#633 [P0] Manual full-system FSV + hardening gate** after 5-Sherlock audit (#629). Children #634-#651 (17 sub-tasks). Per **#655**, Rust `*_fsv.rs` examples are being retired; closure evidence is manual operator SoT readback (read bytes BEFORE + trigger + read bytes AFTER + ≥3 edge classes; see `06 §1.9`).

Open gate-blocking hardening items (active ranks 2 and 4 of new critical path plus carry-over P0s):

| Issue | Defect |
|---|---|
| #618 | Aggregate OOD over flat 5,120-byte buffer (doctrine §1.5ter) |
| #619 | `forward_inverse_inner` flat `Linear(PANEL_DIM, hidden)` projections (doctrine §1.5ter) |
| #620 | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook not invoked from training |
| #621 | Hardcoded `gtau_pass_rate: 1.0` published to weekly dashboard |
| #695 | Rank-lift Goodhart metric must be removed from the ship gate |
| #709 + #715 | Promote inter-embedder distinctness proof and rebuild the static AST forward cache after #704/#714; #670/#704 are closed decisions/evidence |

**#372 aiwonder ZFS root migration CLOSED 2026-05-22.** `/zfs/archive/contextgraph` → `archive/contextgraph` ZFS dataset; `/zfs/hot/contextgraph` → `hotpool/contextgraph` ZFS. Legacy mounted-drive paths (`/mnt/d`, `/mnt/c`) forbidden in active code/FSV. Enforced behaviorally by `scripts/ci/check_no_c_drive_paths.sh` + the #612 → #613 → #614 → #615 → #631 → #632 → #641 → #666 audit chain.

---

## ⚠ Synthetic-vector remediation (2026-05-19/20) — historical context

Between Phase G ramp and 2026-05-19 22:45 UTC, **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of real frozen `E*Instrument` encoders. Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452) document and fix this — all closed 2026-05-19/20.

**Survived, with post-Sherlock caveat:**
- 399,777 AST chunks + 391-shard packed forward cache with real Candle CUDA forward and fail-closed `"no deterministic vector fallback is permitted"` guards. The historical v1 cache is retained as evidence, but is not a clean ship substrate: `e2/e3/e4` collapsed, and legacy `e8` aliased to `e1`. Current static AST cache policy is 9 active slots (`e1,e6,e7,e8,e9,e10,e12,e13,e14`) after #704/#714, with the v2 rebuild tracked by #709/#715.
- Anything deterministically derived from the real v1 cache: #388 cross-embedder tensors, #389 per-slot calibration, #413 Level-0 labels + 491-row accepted registry, #414 skill substrate (128 / 36,773 / 10,579), #404 hierarchy (8 / 157 / 379). These are retained for lineage and diagnosis; gate-countable label/calibration artifacts must be recomputed after #709/#715 v2 cache.
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

**Current:** raw Pearson `0.4843192995` (rank-safe `0.6916886568`; rank-lift `0.7942190170` is exemplar-similarity composite, not predictor — #695). **Honest gap:** `0.4657` against raw Pearson; rank-lift not gate-eligible. **Training/checkpoint lineage not countable through the public trainer** until #683 closes.

---

## Critical-path one-liner

**P0 blocker:** training/checkpoint lineage is not countable through the public trainer and production must fail closed without a manifest (#683). Per pinned context **#712**, the new evidence-weighted order is:

```
#683 trainer/checkpoint lineage unified ⇒ production manifest required
  ↓
#709 + #715 prove distinctness and rebuild 9-slot static AST cache
  ↓
#615 delete_test_call Docker batch ⇒ weak-cell support
  ↓
#618 + #619 doctrine repairs (per-slot OOD; slot-preserving projections)
  ↓
#695 replace rank-lift with raw-Pearson + isotonic
  ↓
#620/#621/#622/#684/#685/#686/#687/#688/#689/#690/#692/#696/#699/#705/#706 strip hardcoded/no-op telemetry and dead public surfaces
  ↓
#633/#655/#656/#657 manual hardening and clean-label evidence gate
  ↓
Resume #392 / #511 / #405 / #379 → 0.95 × 4-window gate + Panel B
```

Post-0.95 roadmap stays archived. Active roadmap stops at the gate + Panel B cross-check. Pathway exploration (#416), skill-linkage expansions (#417), info-physics (#511/#512/#514), creativity engine (#516-#520), neuroscience reserve (#538-#545) all wait.

**Neuroscience-inspired reserve (filed 2026-05-20, DO NOT START until current queue concludes):** 8 contingency issues (#538-#545) at `priority:p3` with explicit `DO NOT START` markers. Reassess only after post-queue gap analysis identifies need. See `08 §6.9`.
