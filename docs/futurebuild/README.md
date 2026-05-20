# futurebuild — ME-JEPA-Code Reference

**Last refreshed:** 2026-05-20. **Style:** source-grounded WHAT-IS reference for AI agents. **Doc count:** 8 (down from 31).

Source code: `/home/cabdru/contextgraph/crates/` (22 active workspace crates; no excluded benchmark crate).
Authority hierarchy: **universal doctrine** (`docs2/AICodingAgentSuperPrompt.md`) > **project addendum** (`CLAUDE.md`) > **this suite**.

---

## Read order

| # | Doc | When to read it |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time touching the system. What it is, tech stack, entry points, subsystem summary, exception hierarchy. |
| 2 | [`02_source_map.md`](02_source_map.md) | Need to know which crate / file owns what. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. 200+ RocksDB column families. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling or implementing an MCP tool. 201 tools (54 are the `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Working on the predictor: embedders, instruments, panels, heads, TCT, corpus, training, heal. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating the system: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Want the proof history: shipped task FSV evidence, ablation results, ship-gate progress, audit history. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Picking up work. Current open issues by phase, critical path to the 0.95 ship gate. |

Plus:

- [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract (still authoritative; not archived).
- [`_archived/`](_archived/) — read-only history of the original 31-doc planning suite (do not read for current state; see `_archived/STOP_READING.md`).

---

## One-line system summary (2026-05-19 binary-doctrine reframe)

> ME-JEPA-Code is a **binary reality predictor**: for any AI-generated code change it returns `Pass | Fail` (against the Docker oracle), with a named failure mode + closest exemplars on Fail, and a binary claim-exists check (Q1) + binary shift-event predictions (Q5) as additional grounded surfaces. On top of the binary primitive it produces a **butterfly-effect pathway surface** (#416 / TASK-PY-G-119): the top-K most-probable downstream future trajectories per candidate patch as chains of Q1/Q2/Q5 binary leaves, letting the operator pick which future to realize through code. Human review of AI patches becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` stably × 4 rolling windows on the 300×8 SWE-bench Lite Python corpus **AND** Panel B (non-overlapping encoder triangulation) confirms within 0.05.

The 2026-05-19 reframe is the **only** product surface: binary predictions against ground truth (Q1 bytes/AST + Q2 Docker oracle + Q5 shift log), one explanation surface (Q3 named failure mode + closest exemplar), and **Q4 ambiguous surfaces removed entirely** (frozen as display-only historical data; ambiguous things have no free binary form against reality, so the system does not predict them). The MCP product shows the binary verdict + named explanation to the human/operator-agent; it does not choose the human's goal, does not infer hidden human intent (#397, #358), and does not predict heuristic-against-heuristic surfaces.

Current ship-gate status: **0.866667** vs target 0.95 on Panel A; Panel B has no measurement yet (#405 will produce the first). Phase G critical path drives both. See `08_roadmap_and_open_work.md §3.−1` for the new binary-doctrine ordering of epics #404 (labeling) / #405 (cross-panel) / #406 (mistake loop) / #407 (calibration audit) / #408 (Q4 freeze) / #409 (paper update).

---

## What changed in this refresh (2026-05-19 → 2026-05-20)

**Information-physics framing + 4 new gate-pathway / safety-net issues + stars-as-vectors formalization.** A new operator-driven report at [`/home/cabdru/contextgraph/docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`](../INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md) maps Vopson's second law of information dynamics (AIP Advances 12, 075310, 2022), Gates' self-dual error-correcting codes in supersymmetry (Doran et al. 2008), FlyWire's whole-brain connectome (Nature 634, 2024-10-02), and Wheeler's "it from bit" onto the project's already-locked-in doctrine. Four new GitHub issues capture the gate-pathway / hygiene work and one existing issue (#417) was extended with a stars-as-vectors operator-UX sub-capability:

- **#511 / TASK-IP-001** — Information-entropy auxiliary loss on slot-preserving predictor latent. **On the road to 0.95.** Formalizes Vopson's second law of infodynamics as a trainer loss term; expected to close 1-2 correlation points.
- **#512 / TASK-IP-002** — Self-dual ECC over the 15-slot panel. Replaces #394's heuristic anti-compensation FSV with a Gates-style formal error-correcting code. Hygiene.
- **#513 / TASK-IP-003** — Connectome rich-club analysis over the #388 cross-embedder correlation tensor. **On the road to 0.95.** FlyWire 2024-style network topology features added to #390's intelligence head; expected to close 1-3 correlation points if topology has predicted structure.
- **#514 / TASK-IP-005** — Eon-style cold-load behavioural FSV. Safety net asserting predictor behaviour is encoded in weights, not transient training state. Catches training-state Goodhart orthogonal to #405's encoder-matched Goodhart defense.
- **#417 augmentation** — Stars-as-vectors slice adds `mejepa_chunk_as_star` + `mejepa_constellation_membership` MCP tools as read-only navigation over the already-shipped #413 + #414 substrate. Operator UX; no new prediction head.

**The deepest synthesis from the report:** ContextGraph is an information-physics instrument. It builds a frozen, slot-preserving "constellation" of measurements for each AST chunk (the array of 15 per-embedder vectors); uses entropy-decreasing operations (compression, symmetry-seeking, label hierarchy) to extract structure; and predicts binary reality outcomes against ground truth (the only thing reality can render a verdict on). The user's original "stars as vectors" intuition that produced the teleological-constellation primitive is the correct generalization of what the system is doing — formalized as the new §1.5quart in [`01_overview.md`](01_overview.md).

## What changed in the prior refresh (2026-05-18 → 2026-05-19)

1. **31 → 8 docs** condensed for AI agent token efficiency. Originals at `_archived/`.
2. **WHAT-IS rewrite** — every claim now traces to a source file/line. No aspirational content. Planning content compressed into `08_roadmap_and_open_work.md`.
3. **Dedicated proof-history doc** (`07_results_and_benchmarks.md`) — FSV evidence index, ablation results, audit history, benchmark numbers, ship-gate progress.
4. **Aiwonder storage policy** (`/zfs/archive/contextgraph/` durable + `/zfs/hot/contextgraph/` hot) made first-class in `01_overview.md §6-7` and `03_storage_schema.md §5`. Legacy `/mnt/d/contextgraph/` evidence is historical only; #369 closed the revalidation sweep and #412 owns residual path cleanup.
5. **Phase H** (inner-loop retirement) marked complete throughout. `feature = "llm"` triggers `compile_error!` at `crates/context-graph-mcp/src/lib.rs:13`.
6. **EK / FP / RWD epic shipping status** captured in `07 §2.9-§2.11`.

**2026-05-19 closures captured:**

- **#388 / TASK-PY-G-088** — Real aiwonder cross-embedder constellation tensors. 516 per-cell/window/slot rows + 2,838 per-cell/window upper-triangle pair tensors written to `/zfs/archive/contextgraph/corpus/constellation-tensors-v1/`.
- **#389 / TASK-PY-G-089** — Per-slot calibration + OOD guard live. `RealityPrediction.per_slot_ood_reasons` now persists per-slot/embedder/pair OOD reasons through `CF_MEJEPA_LIVE_PREDICTIONS`; exposed by `mejepa_explain_prediction` / `mejepa_inspect_prediction`; durable calibration state under `/zfs/archive/contextgraph/state/calibration/per-slot-v1/`.
- **#393 / TASK-PY-G-093** — Capability-discovery ablation suite ran on #396 arrays. All ship-gate-eligible ablations demoted vs the local baseline ladder; flat-vector baseline marked comparison-only.
- **#394 / TASK-PY-G-094** — Anti-compensation FSV suite: corrupted/missing/stale/dimension-shifted slots, contradictory pair relationships, and temporal jumps remain non-ship-gate-eligible even when aggregate normalized scores would pass.
- **#398 / TASK-PY-G-097** — Calibrated constellation ablation rerun. Result `no_promotion`: calibrated full constellation Brier `0.7539…` worse than best local baseline Brier `0.00855…`. The richer slot/pair calibration readout is architecturally correct but does not close the 0.083 correlation gap by itself.
- **#413 / TASK-PY-G-117** — Deterministic auto-label discovery and usefulness scoring completed on aiwonder. Full FSV passed at `/zfs/archive/contextgraph/fsv/task-py-g-117-auto-label-discovery-usefulness-fsv/python_auto_label_discovery_fsv.json`; outputs at `/zfs/archive/contextgraph/state/labels/task-py-g-117-auto-label-discovery-v1/` include 399,777 chunk labels, 3,000 code-state constellations, 1,139 failure evidence sets, 549 usefulness metrics, and 491 accepted label registry rows. Slot identity is preserved; no flat 5,000+d concat is used.
- **#276 / TASK-PY-G-057** — Live-session OOD harvest pipeline shipped. `CF_MEJEPA_OOD_HARVEST` + `CF_MEJEPA_OOD_CALIBRATIONS` + 4× active-learning projection + `OOD_GATE_OVER_FLAGGING` flag.
- **#129 / TASK-PY-G-014** — Calibrated OOD serving gate shipped. Strict `mejepa_verify` now consumes selected `CF_MEJEPA_OOD_CALIBRATIONS`, emits `OOD_SCORE_ABOVE_THRESHOLD`, fails closed on missing/corrupt calibrator via `OOD_CALIBRATOR_MISSING`, preserves cold-cell precedence, and has aiwonder FSV under `/zfs/archive/contextgraph/fsv/task-py-g-014-ood-gate-fsv/run-20260519T104711Z/`.
- **#130 / TASK-PY-G-015** — Gτ guard audit shipped. The audit consumes #388 tensors plus #389 calibration state, reports all 8 Python cells, preserves slot identity (`flat_vector_concat_used=false`), and emits remediation for every low-pass/missing neighborhood. Result is intentionally non-countable: 6 low-pass cells, 1 missing cell (`python:compile_error`), 8 per-cell calibration fail-closed cells.
- **#263 / TASK-PY-G-044** — Multi-head contradiction calibration shipped. `CF_MEJEPA_CONTRADICTION_THRESHOLDS` now stores per-cell `(tau_oracle, tau_failure_count)` thresholds; verdict assembly abstains on calibrated Q2/pass vs high-severity Q3/Q4 conflict, fails closed on missing thresholds, and records `SINGLE_HEAD_ONLY` when contradiction is impossible.
- **#390 / TASK-PY-G-090** — Consensus/contradiction intelligence heads shipped. `RealityPrediction.constellation_intelligence` now carries slot-pair consensus, contradiction, redundancy, novelty, blind-spot, and consequence-pressure evidence; `CF_MEJEPA_CONSTELLATION_PATTERNS` stores promoted relationship patterns; high-value disagreement enters `CF_MEJEPA_ACTIVE_LEARNING_QUEUE`. aiwonder FSV found a real e7/e9 zero-fraction pairwise signal with `+0.18390805` AUC lift over the own-slot baseline.
- **#377 / TASK-PY-G-078** — Q4 trust gate shipped. `mejepa_predict_latest`, `mejepa_explain_prediction`, and `mejepa_inspect_prediction` now expose `q4.rawObservations`, `q4.trustGate`, and `q4.trustedConsequences`; raw Q4 fields remain display-only until producer, calibration, and exact 12-active-slot evidence exists.
- **#266 / TASK-PY-G-047** — Q4 perf label producer shipped and was expanded by #401. Real bounded benchmark/cProfile commands now write 260 usable `CF_MEJEPA_Q4_PERF_LABELS` rows on aiwonder; #127 calibration now makes perf Q4-ready.
- **#399 / TASK-PY-G-098** — Q4 perf real-analyzer re-proof closed. The re-proof wrote `/zfs/archive/contextgraph/fsv/task-py-g-098-q4-perf-real-analyzer-fsv/q4_perf_real_analyzer_fsv.json`, real benchmark/cProfile raw wrappers, real `.prof` artifacts, and independent `ldb` readback for `task-py-g-098-1779173936485:label:test_linear`; the shared Q4 catalog now uses the expanded #266/#401 producer root.
- **#267 / TASK-PY-G-048** — Q4 accuracy label producer shipped and was expanded by #401. Real bounded metric commands now write 240 usable `CF_MEJEPA_Q4_ACCURACY_LABELS` rows on aiwonder; duplicate same-metric source tests and quarantine rows are preserved; #127 calibration now makes accuracy Q4-ready.
- **#268 / TASK-PY-G-049** — Q4 cost label producer shipped and was expanded by #401. Real bounded cost commands now write 240 usable `CF_MEJEPA_Q4_COST_LABELS` rows on aiwonder; pytest-compatible wall-clock, requirements, and wheel metadata deltas feed `PredictedCostRegression`; #127 calibration now makes cost Q4-ready.
- **#400 / TASK-PY-G-099** — Q4 cost real-artifact re-proof closed. The re-proof wrote `/zfs/archive/contextgraph/fsv/task-py-g-099-q4-cost-real-artifact-fsv/q4_cost_real_artifact_fsv.json`, real pytest/pip-wheel raw output, 31 raw cost artifacts, 93 `CF_MEJEPA_Q4_COST_LABELS` rows, and independent `ldb` readback for two same-row/different-chunk dependency-count keys; the shared Q4 catalog now uses the expanded #268/#401 producer root.
- **#269 / TASK-PY-G-050** — Q4 reasoning-class label producer shipped. Observed agent-prose / oracle-outcome harvest fixtures wrote 300 `CF_MEJEPA_Q4_REASONING_LABELS` rows plus producer-heldout `CF_MEJEPA_HEAD_CALIBRATIONS` rows; #127 calibration plus #389 exact-slot evidence now makes the reasoning head Q4-ready.
- **#270 / TASK-PY-G-051** — Q4 non-trivial-on-Pass producer metric shipped. The FSV sampled 500 real aiwonder Oracle-Pass corpus rows, wrote durable `CF_MEJEPA_Q4_PASS_ANNOTATIONS` rows, rendered `weekly/q4_pass_nontrivial.md`, and independently read the canonical annotation key with `ldb` under `/zfs/archive/contextgraph/fsv/task-py-g-051-q4-non-trivial-on-pass-fsv/`; #127 calibration plus #389 exact-slot evidence now makes the non-trivial-on-Pass head Q4-ready.
- **#127 / TASK-PY-G-012** — Q4 confidence calibration shipped and was rerun after #401. `CF_MEJEPA_HEAD_CALIBRATIONS` now stores typed `q4cal::<head>::<class>` records; perf, accuracy, cost, reasoning, and non-trivial-on-Pass are ready with exact #389 active-slot evidence.
- **#401 / TASK-PY-G-100** — Q4 harvest expansion closed. Aggregate Q4 trust is now `q4TrustDecisionEligible=true` for the five producer-backed heads; direct reopened RocksDB readback proves one row each from `CF_MEJEPA_Q4_PERF_LABELS`, `CF_MEJEPA_Q4_ACCURACY_LABELS`, and `CF_MEJEPA_Q4_COST_LABELS`. Security and structural heads remain excluded/fail-closed until #402 adds real producers.
- **#383 / TASK-PY-G-084** — Locked-in rule (pinned `type:context`): gate-active issue order must be evidence-weighted, not vision-weighted. `08 §3.0` now leads with this and reorders the critical path: train missing Q4/Q5 label producers + calibrate OOD on harvested data BEFORE further slot-preserving predictor refactors.
- **Newly filed since 2026-05-17** captured in `08`: PY-G-076/077/078/079/080/081/082/083/084/100/101, AIWONDER-019/020/021. Migration p0/p1 follow-ups (#372 ZFS mount, #380 off-host backup, #384 CUDA rollback) included.

Memory of *why* this refresh happened lives in operator-feedback memory:
`~/.claude/projects/-home-cabdru-contextgraph/memory/feedback_docs_rewrite_no_delegation_condense.md`.
