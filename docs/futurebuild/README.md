# futurebuild — ME-JEPA-Code Reference

**Style:** source-grounded WHAT-IS reference for AI agents. **Doc count:** 8.

Source code: `/home/cabdru/contextgraph/crates/` (22 active workspace crates).
Authority hierarchy: **universal doctrine** (`docs2/AICodingAgentSuperPrompt.md`) > **project addendum** (`CLAUDE.md`) > **this suite**.

---

## ⚠ 2026-05-22 hardening pivot — read first

On 2026-05-22 the operator opened **#633 / [P0] Manual full-system FSV and hardening gate before new work** and a 5-Sherlock parallel forensic audit (closeout #629) shipped ~40 P0 closures plus ~15 new P0 findings. **Gate-moving work on #392 / #511 / #405 is suspended** until the #633 child queue resolves. The critical path now stops at the hardening gate:

```
#633 manual full-system FSV (children #634-#651, 17 sub-tasks)
 → #655 retire Rust FSV examples in favour of manual SoT (in-progress p0)
 → #656 mejepa-heal-drill conformal-overcoverage repair (in-progress p0)
 → #657 TASK-PY-G-136 per-embedder geometry-label bakeoff (in-progress p0)
 → resume #615/#616 data-substrate work, then #392 trajectory ECE
```

Operational change. Per #655, **Rust `*_fsv.rs` examples are being retired**. Closure evidence is now manual operator SoT readback only — read the actual bytes/rows/files/services before AND after each trigger, paste both into the issue comment, exercise happy path + ≥3 edge classes. Return values, green scripts, and previous FSV JSON are supporting evidence only, not the verdict. See `06_operations_and_fsv.md §1.9` for the binding contract.

**Doctrine-violation P0s opened by the 5-Sherlock audit (open, must close before gate work resumes):**

- **#618 / TASK-PY-G-136** — aggregate OOD score uses flat 5,120-byte packed panel buffer (doctrine §1.5ter violation)
- **#619 / TASK-PY-G-137** — `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter violation)
- **#621 / TASK-PY-G-138** — hardcoded `gtau_pass_rate: 1.0` published to MCP weekly dashboard
- **#670** — regenerate or retire invalid forward-cache spaces `e2/e3/e4/e8` (forward-cache embedder collapse uncovered by #666 closure)

**#372 aiwonder ZFS root migration CLOSED 2026-05-22** — `/zfs/archive/contextgraph` now backs the `archive/contextgraph` ZFS dataset; `/zfs/hot/contextgraph` backs `hotpool/contextgraph` ZFS. The aiwonder root readiness blocker is satisfied; legacy mounted-drive paths (`/mnt/d/contextgraph/`, `/mnt/c/`) are forbidden in active code and FSV. `scripts/ci/check_no_c_drive_paths.sh` plus the per-data-substrate audit chain (#612 → #613 → #614 → #615 → #631 → #632 → #641 → #666) enforce this behaviorally.

---

## ⚠ Synthetic-vector remediation (2026-05-19 / 20) — historical context

Between Phase G ramp and 2026-05-19 22:45 UTC, the **inference path** in `crates/context-graph-mejepa/src/compiler.rs::vectors_for` filled instrument-panel slots with SHA256-seeded synthetic vectors via `deterministic_vector(slot, &digest)` instead of calling the real frozen `E*Instrument` encoders. Six P0 issues (#422 / #424 / #426 / #437 / #438 / #441 / #452, all closed 2026-05-19/20) document and fix this contamination across the inference panel, project cache, FSV deterministic harness, project ingest, trained-checkpoint pipeline, trainer weight update, and native active-embedder adapter.

**What survived the bug:**

- 399,777 AST chunks + 391-shard packed forward cache + 12 active embedders (real Candle CUDA forward; ~24h GPU run; SHA-pinned safetensors; fail-closed `"no deterministic vector fallback is permitted"` guards in `crates/context-graph-mejepa-embedders/src/forward.rs`)
- Anything deterministically derived from the real cache: #388 cross-embedder tensors, #389 per-slot calibration, #413 Level-0 labels + 491-row accepted registry, #414 skill substrate (128 / 36,773 / 10,579), #404 hierarchy (8 / 157 / 379)
- All structural / invariant work: slot-preserving doctrine, anti-compensation, ambiguity boundary, mistake-loop machinery, runtime ability resolver

**What is invalidated:**

- The **0.866667** ship-gate measurement (came from inference path through synthetic vectors + untrained predictor + `MEJEPA_BEST_STUB_V1` checkpoint)
- The `λ_inv ∈ {0, 0.25, 0.5, 1.0}` ablation (every variant measured 0.8666666746139526 — same synthetic input)
- Original #393 capability-discovery ablation suite + #398 calibrated rerun (pre-fix Brier 0.7539 was against synthetic-input predictor; real-input reruns now exist and remain `no_promotion`)
- The "slot-preserving architecture doesn't close the 0.083 gap by itself" claim (pre-fix claim invalid; real-input #393/#398/#546 reruns are negative evidence, not promotion evidence)
- The framing that "the 0.866 → 0.95 gap is a function of labels, not embedders/storage" (invalid as a broad causal claim; real-input constellation/λ_inv ablations did not identify those surfaces as gate movers)

**Post-fix baseline:** aiwonder FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` is the current Panel A measurement of record: raw `prediction_oracle_correlation = 0.4843192995`, rank-safe calibrated `0.6916886568`, rank-lift calibrated from real code-state constellations `0.7942190170`; report sha256 `8a018eaf63fab34eb7c3bf8d60a5b9f92d04a58d99a702b9fc462b21236ba499`. Earlier row-trace run `run-20260520T185945Z` preserves #536 metrics and adds row-level calibration/holdout raw, rank-safe, and rank-lift prediction traces for #549. #547 revalidated that the selected rank-lift policy is split-disjoint and not holdout-selected. #548 then reproduced #536 exactly and accepted typed blocker `MIXED_CELL_CALIBRATION_UNDERPOWERED`. #549 formally proved repo/path metadata plus calibration-selected raw/rank-safe/rank-lift score fusion are useful but insufficient. #392 aggregate trajectories first lifted to `0.8008320332`; file-path plus skill/ability trajectories lifted to `0.8422425985`; sparse out-of-fold probability bins regressed ECE. The hardened support-aware slice at `/zfs/archive/contextgraph/fsv/task-py-g-392-temporal-trajectory-features-fsv/run-20260520T214049Z/` selected a split-legal global mean-bias correction over the affine file/skill surface: `0.8497869968` correlation, Brier `0.0697008073`, ECE `0.0299093761`, and accuracy `0.9083333611`, with exact #536 row-trace reproduction and no mixed-cell threshold degeneracy. The FSV also recorded skill coverage explicitly: `1531 / 3000` expected code-state keys are covered and `1469` are deliberately marked uncovered, not silently zero-filled. #554 proved usefulness-weighted skills carry stronger consequence signal but were not active against the old artifact. #558 then rematerialized the full v2 label corpus (`399777` chunk rows, `398215` episodes, `157` skills, `9489` memberships) and reran #392 from `/zfs/archive/contextgraph/fsv/task-skill-006-full-corpus-skill-rematerialization-fsv/`. Before the later #551/#392 correction, that rerun selected `usefulness_weighted_combined`: correlation `0.8679359555`, Brier `0.0620564893`, ECE `0.0396374688`, accuracy `0.9083333611`, pass recall `0.9166666865`, fail recall `0.9000000`, with report sha256 `2c3c7cea2ced0afc4715bad03624401ea497cfee1260d62ce7f305f5fb11828a`. #560 then tried split-legal ECE-focused mean-bias repair against the rematerialized skill DB and reproduced the same legal best (`0.8679359555` correlation, Brier `0.0620564893`, ECE `0.0396374688`; report sha256 `c2505eeb6f755d362510b61d78437f364029deb000363d54715ea3238eec6eee`). #557 adds the skill usefulness audit surface that makes source-byte sequences, lifecycle/demotion history, generic high-support rejections, and blanket-expansion regressions visible before further ECE work. That was a strong trajectory signal, but it remained non-promotable because ECE was still above #536's `0.0240316074`; typed blockers `TEMPORAL_FILE_SKILL_TRAJECTORY_ECE_BLOCKED` and `TASK_SKILL_002_USEFULNESS_WEIGHTED_NONREGRESSING_BUT_ECE_BLOCKED` remained accepted. The later #551/#392 correction supersedes this as the current #392 state.

**2026-05-21 correction (#551/#392).** The #551 coverage-gap hypothesis is resolved as a justified negative-control finding, not an active coverage repair. Direct aiwonder readback showed all `1139 / 1139` failure-evidence rows are already skill-covered; the `1469 / 3000` uncovered code states are pass/context-only. Blanket pass/context expansion improves raw coverage but regresses #392, so coverage cap expansion is not the repair. The corrected #392 run after removing evidence-consequence double-calibration is `/zfs/archive/contextgraph/fsv/task-py-g-392-skill-kind-evidence-fixed-base-fsv/run-20260521T053605Z/` with report sha256 `9ed8830f184e0aca07a1ee47897b31b342dbdaf2db0bacc28d0a08981dfbc29c` and readback sha256 `9d8f73a794a98b84ed7a4d89e7c9c3037b94b0e5bf27fe0d3c9f94a8e436a9c3`. Legal best was `calibration_selected_mean_bias_calibration`: correlation `0.8770877719`, Brier `0.0581485741`, ECE `0.0475411229`, accuracy `0.9250000119`. The later #392 evidence-semantics hardening run `/zfs/archive/contextgraph/fsv/task-py-g-392-evidence-semantics-hardening-fsv-release/run-20260521T074421Z/` standardizes cause-residual features and proves cause-bin actually applies to calibration/holdout rows. Legal best is now `calibration_selected_cause_bin_calibration`: correlation `0.8939641118`, Brier `0.0504463837`, ECE `0.0402133949`, accuracy `0.9375`. The follow-on final-bin diagnostic `/zfs/archive/contextgraph/fsv/task-py-g-392-final-bin-guarded-fsv/run-20260521T090040Z/` is a retained negative result: it slightly improved correlation/Brier but worsened ECE and is explicitly forbidden from legal-best selection, so cause-bin remains selected. The latest ship-gate FSV `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/` folds #392 active/abstain in as a first-class diagnostic: `205 / 240` active rows, active corr `0.9526350498`, Brier `0.0231730901`, ECE `0.0169885978`, but full active/abstain gate remains false due per-cell active metric, rolling-window, and statistical-power blockers. #392 remains active.

---

## Read order

| # | Doc | When to read it |
|---|---|---|
| 1 | [`01_overview.md`](01_overview.md) | First time touching the system. What it is, tech stack, entry points, subsystem summary, ambiguity boundary. |
| 2 | [`02_source_map.md`](02_source_map.md) | Need to know which crate / file owns what. Dependency graph. Binary targets. |
| 3 | [`03_storage_schema.md`](03_storage_schema.md) | Touching durable state. 200+ RocksDB column families. aiwonder storage policy. |
| 4 | [`04_mcp_tools.md`](04_mcp_tools.md) | Calling or implementing an MCP tool. 214 tools (67 are the `mejepa_*` product surface). |
| 5 | [`05_predictor_pipeline.md`](05_predictor_pipeline.md) | Working on the predictor: embedders, instruments, panels, heads, TCT, corpus, training, heal, mistake loop. |
| 6 | [`06_operations_and_fsv.md`](06_operations_and_fsv.md) | Operating the system: FSV protocol, runbook, DR, observability, test suite. |
| 7 | [`07_results_and_benchmarks.md`](07_results_and_benchmarks.md) | Proof history: shipped task FSV evidence index, ship-gate progress, ablation results. Marks pre-fix evidence INVALID. |
| 8 | [`08_roadmap_and_open_work.md`](08_roadmap_and_open_work.md) | Picking up work. Current open issues, critical path to the 0.95 ship gate, rebaseline blocker, post-ship plans. |

Plus:

- [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) — mandatory verification contract.
- [`_archived/`](_archived/) — read-only history (do not read for current state).

---

## One-line system summary

> ME-JEPA-Code is a **binary reality predictor**: for any AI-generated code change it returns `Pass | Fail` (against the Docker oracle), with a named failure mode + closest exemplars on Fail (Q3), a binary claim-exists check (Q1) and binary shift-event predictions (Q5) as additional grounded surfaces. On top of the binary primitive it surfaces a **butterfly-effect pathway tree** (#416): top-K (3–5) most-probable downstream futures per candidate patch as chains of Q1/Q2/Q5 binary leaves, letting the operator pick which future to realize.

Human review of AI patches becomes statistically unjustified once `prediction_oracle_correlation ≥ 0.95` × 4 rolling windows on the 300×8 SWE-bench Lite Python corpus **AND** Panel B (non-overlapping encoder triangulation, #405) confirms within 0.05.

**Current ship-gate status:** binary rank-lift remains `0.7942190170` from real code-state constellation labels (`0.6916886568` rank-safe, `0.4843192995` raw) in aiwonder run `/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`. #392 active/abstain is diagnostic-only for now: aggregate active rows clear 0.95, but cell/window/power gates still fail. **The shipped 0.7942 figure was measured before the 5-Sherlock audit; the manual FSV gate (#633) must revalidate it under manual operator SoT before any new gate-moving work resumes.** The previous 0.866667 figure was measured against a synthetic-input inference pipeline (see synthetic-vector warning below) and is invalid.

---

## Critical-path one-liner (read `08` for detail)

**P0 blocker:** the **2026-05-22 hardening gate (#633)**. Until its child queue (#634-#651) is RESOLVED or BLOCKED with manual SoT evidence, no further feature/gate work is admissible. Per #383, gate-active issue order is evidence-weighted, and the current evidence is "manual SoT verification is missing across host/runtime/storage/GPU/Docker/MPS, the predictor product path, MCP tools, slot-preserving panels, RocksDB CFs, trainer + mistake loop, CLI + hooks + shift log, build + tests + CI, observability, performance, security, documentation, benchmark honesty, and operational practice."

Critical path (current → ship):

```
#633 manual full-system FSV + hardening gate (in-progress)
  ├── #634 aiwonder host/ZFS/GPU/Docker/MPS/env
  ├── #635 aiwonder model cache + no-download embedder readiness
  ├── #636 300x8 corpus + AST chunks + forward cache + labels + skills
  ├── #637 SWE-bench Docker oracle + weak-cell truth
  ├── #638 ME-JEPA RealityPrediction Q1/Q2/Q3/Q5 product path
  ├── #639 MCP product tools + retired surfaces + JSON-RPC state
  ├── #640 slot-preserving panels + frozen-instrument execution
  ├── #642 RocksDB CF registry + data integrity + retention + restore
  ├── #643 trainer + mistake loop + replay + checkpoints + Fisher/EWC
  ├── #644 CLI + hooks + shift log + path discipline + no mounted-drive
  ├── #645 build + tests + clippy + CI + secrets + SBOM/provenance
  ├── #646 observability + evidence retention + backup + DR
  ├── #647 performance + scalability + capacity + cost
  ├── #648 security hardening for code + aiwonder ops
  ├── #649 documentation + GitHub issue ledger truth
  ├── #650 benchmark + gate-metric honesty
  └── #651 operational practice + RCA discipline
  ↓
#655 retire Rust FSV examples → manual SoT guidance (in-progress p0)
#656 mejepa-heal-drill conformal-overcoverage repair (in-progress p0)
#657 TASK-PY-G-136 per-embedder geometry-label bakeoff (in-progress p0)
  → #661 cross-embedder label consensus map
  → #662 gate-safe geometry-label promotion
  → #664 geometry region dictionary + per-embedder search router
  → #665 new prediction heads over per-embedder geometry labels
  ↓
Doctrine repairs (must close before gate resumes):
  #618 aggregate OOD over flat 5,120 buffer (doctrine §1.5ter violation)
  #619 forward_inverse_inner flat Linear projections (doctrine §1.5ter violation)
  #621 hardcoded gtau_pass_rate=1.0 in HoldoutReport
  #670 invalid forward-cache spaces e2/e3/e4/e8
  ↓
Data-substrate completion:
  #615 delete_test_call assertion-duty regeneration (in-progress p0)
  #616 weak-cell oracle timeout/error reruns
  #671 stale SWE-bench harness defaults
  #672 geometry scalar feature lineage gate
  #673 label-quality vs noise audit
  #674 run-remote wrapper hang
  ↓
Resume #392 trajectory ECE repair → #405 Panel B → 0.95 ship gate
```

Post-0.95 roadmap stays archived/deferred. The active roadmap stops at the 0.95 × 4-window ship gate plus Panel B cross-check; pathway exploration, remaining skill-linkage expansions, info-physics formalization, and creativity-engine work do not take priority until that gate is met.

**Neuroscience-inspired reserve (filed 2026-05-20, DO NOT START until current queue concludes):** eight contingency issues (#538-#545) for if the current queue + creativity engine fail to close the gap — predictive-coding head, schema-prioritized replay, hierarchical predictive coding, mixed-selectivity binding, population-dynamics trajectories, sparse representations, synchrony binding, idle-time rollouts. All at `priority:p3` with explicit `DO NOT START` markers. Reassess only after post-queue gap analysis identifies need. See `08_roadmap_and_open_work.md §7`.
