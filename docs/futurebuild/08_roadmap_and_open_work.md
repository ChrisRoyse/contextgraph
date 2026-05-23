# 08 — Roadmap and Open Work

**Source:** `gh issue list --state open`, recent `type:decision` closes, git log, GitHub issue comments.

---

## 0. Doctrine reminder (binary contract)

The prediction contract is **three binary surfaces against ground truth** (Q1 bytes/AST, Q2 Docker oracle, Q5 shift log), **one explanation surface** (Q3 named failure mode + closest exemplar), **Q4 removed entirely** (ambiguous heads not predictable). See `01 §1.1`, `05 §0`.

---

## 1. Current ship gate

```
prediction_oracle_correlation ≥ 0.95
  stable × 4 consecutive rolling windows
  per mathematically-valid mixed (mutation_category × language) cell
  with all-pass/all-fail cells using #532 degenerate-cell replacement metric
  on 300×8 Python SWE-bench Lite corpus
  using Q2 Docker-oracle agreement as binary verdict
  AND cross-panel oracle correlation ≥ 0.95 on Panel B
  AND |corr_panel_a − corr_panel_b| ≤ 0.05
```

**Latest measurement:** raw Pearson `0.4843192995` from real code-state constellation labels in `fsv/swebench-300x8-ship-gate-fsv/run-20260521T132308Z/`. Brier `0.0923729688`, ECE `0.0240316074`.

**Honest gap to ship:** `0.4657` against raw Pearson, plus four-window stability + Panel B cross-checks.

**Degenerate-cell rule (#532).** `compile_error::python`, `wrong_file::python`, any future single-class cell cannot honestly use Pearson. FSV must emit cell's variance class, replacement gate metric (`all_fail_recall_confidence_brier_ece` or `all_pass_recall_confidence_brier_ece`), and statistical-power diagnostics instead of a bare "correlation unavailable" blocker.

**P0 product contract (#397):** ME-JEPA is the binary reality predictor. Returns `Pass | Fail` per candidate change against Docker oracle (Q2), named-failure-mode + closest-exemplar explanation (Q3) on Fail. Q1 (claim-exists) + Q5 (predicted-shift-event) are additional binary predictions. Q4 ambiguous surfaces out of scope.

Once met → human review of AI-generated Python patches is statistically unjustified. Auto-trust policy gates engage (#277).

---

## 2. Open issue snapshot (live tracker is authoritative)

As of 2026-05-23 live tracker readback: **158 open** issues across the workspace.

By phase:

| Label | Open count |
|---|---|
| `phase:g` | 138 (critical path and blocked tail) |
| `phase:migration` | ~5 (#372 closed; tail items remain) |
| `phase:f-followup` | 1 |
| `phase:f` / `phase:h` | 0 (closed) |

By priority (excerpt):

| Priority | Examples |
|---|---|
| `priority:p0` | **#712** Sherlock critical-path context, **#683** trainer/checkpoint lineage, **#633** manual full-system FSV gate (parent), #634-#651 sub-tasks, **#655** retire Rust FSV (in-progress), **#656** mejepa-heal-drill conformal-overcoverage (in-progress), **#657** TASK-PY-G-136 geometry-label bakeoff (in-progress), **#615** delete-test-call regeneration (in-progress), #616 weak-cell oracle reruns, #618/#619/#620/#621/#622 doctrine/trainer-surface P0s, **#685/#687/#688/#690/#699** hardcoded/no-op telemetry P0s, **#695** rank-lift Goodhart metric removal, #661/#662 geometry-label promotion (blocked behind #657), **#709/#715** v2 static-cache distinctness/rebuild, **#397** consequence-prediction contract (context pin), **#383** evidence-weighted ordering (context pin), #167 burn-in (blocked by #633), #165 confidence gate (blocked by #167), #310 META context |
| `priority:p1` | #593 explicit train/serve contract, #684/#686/#692/#696/#705/#706 non-countable OOD/Q5/trainer-adjacent telemetry repairs, #630 long-source/OOM forward-cache quarantine recovery, #626 hygiene retention 90 CFs missing, #664 geometry region dictionary (blocked behind #657), #380 off-host backup, #379 multi-file holdout, #416 pathway, #417 skill linkage remaining, #511 entropy loss, #433 ability resolver replay completeness, #442 librocksdb-sys native build, #693 Phase H absence-check script/doc drift |
| `priority:p2` | EK series tail, post-ship policies, #382 frozen-panel dimensionality ablation, #378 tool-usage telemetry, #623/#624 minor doctrine string drift, #665 new prediction heads over geometry labels |
| `priority:p3` | Deferred (SWE-bench Verified subset, v2 stretch, future Docker image plan), 8× neuroscience-reserve issues (#538-#545) |

---

## 3. Critical path (#712 — pinned authoritative ordering)

Per pinned contexts #383 (LOCKED — gate-active issue order must be evidence-weighted) + #712 (2026-05-23).

| Rank | Issue | Action | Expected impact | Status |
|---|---|---|---|---|
| 1 | **#683** | Unify real training/checkpoint export with public trainer contract; require production `trained_checkpoint_manifest_path` at `mejepa/src/cli.rs:307-314` fail-closed | Largest; current checkpoint evidence is out-of-band from the public trainer | open p0 |
| 2 | **#709 + #715** | Add/promote inter-embedder distinctness gate, then rebuild a v2 static AST cache with active slots `e1,e6,e7,e8,e9,e10,e12,e13,e14`; `e2/e3/e4` remain retired for static AST by #704/#714 | Replaces invalid v1 cache with a clean 9-slot substrate; proves post-`ff4d4c03` `e8` is not an `e1` alias | open p0 |
| 3 | **#615** | delete_test_call Docker oracle batch over 1,131 deterministic candidates → rebuild AST → #614 mask → labels → #392 → #611 | Closes `0.69 → ≥0.85` weak-cell ceiling | in-progress p0 |
| 4 | **#618 + #619** | Per-slot OOD aggregation at `compiler.rs:502`; slot-preserving panel projections at `predictor/mod.rs:136-147` | Doctrine §1.5ter repair; removes anti-compensation cheat path | open p0 |
| 5 | **#695** | Stop using rank-lift as gate metric; use raw Pearson + isotonic/Platt | Reveals honest gap; ECE-safe | open p0 |
| 6 | **#620/#621/#622/#684/#685/#686/#687/#688/#689/#690/#692/#696/#699/#705/#706** | Eliminate hardcoded passing telemetry, pseudo-cert confidence, no-op trainer APIs, non-countable Q5/OOD claims, and exact-panel-only mistake-loop surfaces before future wiring can trust them | Prevents Goodhart on future mistake-loop/trainer wiring | mixed p0/p1 |
| 7 | **#633 + #655 + #656 + #657 and children** | Manual full-system FSV/hardening gate, auto-FSV retirement, heal-drill truthfulness, geometry-label bakeoff | Evidence integrity and substrate prerequisites | mixed |
| Done | **#704/#714/#670/#691/#707** | Static temporal retirement, invalid-space policy, hook fail-visible behavior, strict OOD fail-closed behavior | Closed evidence; retained as prerequisites | closed |
| 8 | **#392 / #511 / #405 / #379** | Trajectory ECE / entropy aux loss / Panel B / multi-file holdout | **Blocked behind active ranks 1-7** | blocked |

### 3.1 Manual full-system FSV gate (#633)

**Children #634–#651 (all `priority:p0`, `status:needs-triage`):**

| Issue | Domain |
|---|---|
| #634 | aiwonder host, ZFS roots, GPU, Docker, MPS, env |
| #635 | aiwonder model cache + no-download embedder readiness |
| #636 | 300×8 corpus, AST chunks, forward cache, labels, skills |
| #637 | SWE-bench Docker oracle + weak-cell outcome truth |
| #638 | ME-JEPA `RealityPrediction` Q1/Q2/Q3/Q5 product path |
| #639 | MCP product tools, retired surfaces, JSON-RPC state |
| #640 | slot-preserving panels + frozen-instrument execution |
| #642 | RocksDB CF registry, data integrity, retention, restore |
| #643 | trainer, mistake loop, replay, checkpoints, Fisher/EWC |
| #644 | CLI, hooks, shift log, path discipline, no mounted-drive |
| #645 | build, tests, clippy, CI, secrets, SBOM/provenance |
| #646 | observability, evidence retention, backup, DR |
| #647 | performance, scalability, capacity, cost |
| #648 | security hardening for code + aiwonder ops |
| #649 | documentation + GitHub issue ledger truth |
| #650 | benchmark + gate-metric honesty audit |
| #651 | operational practice + RCA discipline hardening |

Done When: all linked P0 children have either RESOLVED with manual SoT before/after + happy + 3 edge cases, or BLOCKED with specific blocker issue. Manual SoT contract in `06 §1.9`. Operational implication: Rust `*_fsv.rs` examples are being retired (#655); new gate evidence is captured as issue comments with direct bytes-on-disk readback.

### 3.2 Doctrine-violation P0s (gate-blocking when open)

| Issue | Title | Source |
|---|---|---|
| #618 / TASK-PY-G-136 | Aggregate OOD score uses flat 5,120-byte panel buffer (doctrine §1.5ter) | mejepa |
| #619 / TASK-PY-G-137 | `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter) | mejepa + mejepa-train |
| #620 | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook never invoked from training | mejepa + mejepa-train |
| #621 / TASK-PY-G-138 | Hardcoded `gtau_pass_rate: 1.0` in HoldoutReport published to MCP weekly dashboard | mejepa-train + mcp |
| #622 | Trainer dead-code cleanup: distill + lora_wiring + aux | mejepa-train |
| #623 | `cold_start_conformal_policy` emits `alpha=0.0` (dead public API) | mejepa |
| #624 | `verdict_assembly` inline `CONTRADICTION_CALIBRATION_INVALID` not declared constant | mejepa |
| #625 | `DeterministicPredictor` publicly re-exported | mejepa |
| #626 / TASK-PY-G-139 | Hygiene retention default policy missing ~90 CFs | mejepa-hygiene |
| #627 | shift-subscriber runner integration test fails on main | mejepa-shift-subscriber |
| #628 | Doc/code drift: `MEJEPA_TCT_DIM_MISMATCH` (docs) vs `MEJEPA_INFER_DIM_MISMATCH` (emitted) | docs |
| #685 | `AuxHeadEnsemble::forward_logits` returns zeros regardless of input | mejepa |
| #687 | `detach_loras` is a public no-op that always returns Ok | mejepa |
| #688 | `EpochSummary` hardcodes best/worst category and language | mejepa |
| #690 | `mejepa-heal-drill` hardcodes `retrain_triggered: true` | mejepa |
| #695 | Rank-lift exemplar transformation is a Goodhart magnifier | mejepa |
| #699 | `compute_train_health` reads pseudo-training certificates | mejepa + utml |
| #709 | Add inter-embedder distinctness gate before cache promotion | mejepa-embedders |
| #715 | Rebuild static AST forward cache with 9-slot policy and post-fix E8 | mejepa-embedders + corpus |

---

## 4. Operator-facing surfaces (above the binary primitive)

### 4.1 Pathway exploration surface (#416, OPEN p1)

L3 counterfactual rollouts for decision support — top-K (3–5) most-probable future trajectories per candidate patch as chains of Q1/Q2/Q5 binary leaves only. **Q4 strictly excluded** (`ambiguous_leaves_in_pathway == 0` FSV assertion).

Consumes: #413 Level-0 registry (closest-exemplar attachment), #404 named failure modes (Fail leaves), #406 mistake loop (credits across pathway leaves), #407/#547 calibration disjointness (Goodhart-safe surfacing), #417 skill/chunk attachment per leaf.

Precedes: #335 `mejepa_reverse_prediction` — operator picks pathway, system tells agent what patch-shape lands it.

Not gate-blocking; ships when gate-movers stabilize.

### 4.2 Bidirectional chunk ↔ skill linkage (#417, OPEN p1)

Read-only MCP surfaces shipped: `mejepa_skill_to_code`, `mejepa_code_to_skill`, `mejepa_skill_set_query`, `mejepa_skill_coverage_audit`, `mejepa_chunk_as_star`, `mejepa_constellation_membership`, `mejepa_skill_impact`, `mejepa_skill_graph_inspect`, `mejepa_skill_conflict_graph`, `mejepa_skill_browse`.

#557 extends with skill usefulness evidence: candidate kind, support/fail-pass/lift, genericity, per-cell contribution, live-safe vs target supervision labels, source sequences, lifecycle, uncovered code-state keys, unknown ordered constellations, generic high-support rejections, metric-regression rejections.

Remaining open: cross-repo skill transfer + pathway-leaf attachment.

Constraint: skills are deterministic groupings of accepted Level-0 labels. Read-only navigation over training substrate; no new prediction surface; binary-leaf constraint preserved at prediction boundaries.

---

## 5. Information-physics items (peer-reviewed framing)

Four issues filed against `docs/INFORMATION_PHYSICS_AND_STARS_AS_VECTORS_REPORT.md`:

| Issue | Status | Source principle |
|---|---|---|
| #511 / TASK-IP-001 — Information-entropy auxiliary loss | OPEN p1; blocked candidate after clean raw-Pearson rerun | Vopson 2nd law of infodynamics (AIP Advances 12, 075310, 2022) |
| #512 / TASK-IP-002 — Self-dual ECC over 15-slot panel | OPEN p1; hygiene | Gates' self-dual ECC in supersymmetry (Doran et al. 2008) |
| #513 / TASK-IP-003 — Connectome rich-club analysis | ✅ CLOSED as demoted negative finding | FlyWire connectome (Nature 634, 2024-10-02) |
| #514 / TASK-IP-005 — Eon-style cold-load behavioural FSV | OPEN p1; safety net | Eon Systems fly demonstration (March 2026) |

#513 tested and demoted: topology features remain diagnostic-only; not promoted as predictor inputs (#527 demoted).

---

## 6. Post-0.95 creativity engine (autonomous bounded growth)

**Architectural framing.** ME-JEPA is the *engineering* half (verifies binary claims against reality). The *creativity* half is **autonomous embedder synthesis** with two oracles gating every proposal:

1. **Novelty oracle** — cross-correlation: candidate orthogonal to existing N? Reject if max correlation > τ_novelty (~0.7).
2. **Utility oracle** — ablation: improves per-cell oracle correlation by ≥ δ_utility (~0.005)?

Only candidates passing BOTH promoted. Rejected candidates write `CF_MEJEPA_EMBEDDER_PROPOSAL_AUDIT` row with typed reasons.

**Bounded by reality.** Orthogonal informative axes ContextGraph admits is bounded by intrinsic dimensionality of corpus. For code N* ≈ 50-500. System discovers own N* because novelty oracle rejects every candidate once corpus is fully resolved.

**Five issues (all post-0.95 — none are gate-movers):**

| Rank | Epic | Why | Priority |
|---|---|---|---|
| C-1 | **#517** Mistake-driven embedder demand signal | Closes loop between unresolvable mistakes and embedder synthesis | p1 |
| C-2 | **#516** Novelty + Utility oracle | Two-test gate converts unbounded growth into bounded automated growth | p2 (p1 immediately post-0.95) |
| C-3 | **#518** Embedder retirement audit + Pareto frontier | Vopson 2nd-law applied to embedder set; dominated embedders retire with grace period | p1 |
| C-4 | **#519** Domain-routed sparse panel | Universal ~30 slots + per-domain ~15 extension; 3-5× storage saving | p1 |
| C-5 | **#520** Episodic-to-semantic consolidation | Settled chunks → `(cluster_id, residual)`; 99% storage reduction in high-clustering regime | p2 |

**Sequencing:** C-1 first (demand rows). C-2 (validates proposals). C-3 (closes loop with pruning). C-4 (multi-language scaling). C-5 (steady-state).

**Composition.** Together with v2-stretch creativity tasks (#329 AlphaEvolve outer loop, #335 reverse-prediction, #354-357 EK v2 series), forms a **self-resolving observer**: system grows panel until corpus resolved, prunes dominated embedders, specializes per domain, compresses settled clusters. Bounded by reality, not ambition. Storage bounded; search precision maximal.

**NOT in scope.** No new ambiguous-head prediction surfaces. No consciousness / sentience / phenomenology framing. No simulation-hacking / reality-bending claims.

### 6.9 Neuroscience-inspired reserve (DO NOT START until current queue concludes)

**Status:** all 8 issues filed at `priority:p3`, `status:needs-triage`, with explicit `⚠ DO NOT START — Neuroscience-reserve contingency` markers.

**Activation rule:** *contingency plans* for if the current Phase G queue + creativity engine fail to close the remaining ship-gate gap. **NOT to be started until:**

1. Current #712 queue concludes.
2. Post-queue gap analysis on a clean raw-Pearson, clean-substrate measurement concludes the remaining gap is not closable with the current architecture + corpus + label substrate.
3. Operator explicitly re-prioritizes any (or all) from `p3` based on what the gap analysis identifies.

If the gate closes via the current queue, these issues stay in `p3` reserve indefinitely. If the gate does not close, they become a candidate lever set for the next architectural sweep.

**Tier 1 — high-impact, doctrine-compatible, addresses current gap directly:**

| Issue | Title | Neuroscience source |
|---|---|---|
| **#538 / TASK-PY-NEURO-001** | Predictive-coding head: top-down per-slot prediction + bottom-up residual | Rao & Ballard 1999 (*Nature Neuroscience*); Friston free-energy |
| **#539 / TASK-PY-NEURO-002** | Hippocampal schema-prioritized replay consolidation (incl. reverse replay for pathway credit) | Mattar & Daw 2018 (*Nature Neuroscience*); Liu et al. 2019 (*Cell*) |
| **#540 / TASK-PY-NEURO-003** | Hierarchical predictive coding tied to #404 / #413 / #414 hierarchy | Felleman & Van Essen 1991 (*Cerebral Cortex*); Bastos et al. 2012 (*Neuron*) |
| **#541 / TASK-PY-NEURO-004** | Mixed-selectivity cross-slot binding layer (replaces #390's linear pairwise statistics) | Rigotti et al. 2013 (*Nature*); Fusi et al. 2016 (*Current Opinion in Neurobiology*) |

**Tier 2 — promising but more speculative; design-influence on existing epics:**

| Issue | Title | Composes with |
|---|---|---|
| **#542 / TASK-PY-NEURO-005** | Population-dynamics low-dim trajectories for #416 (Shenoy/Churchland/Cunningham) | #416 pathway; #392 temporal |
| **#543 / TASK-PY-NEURO-006** | Sparse distributed representations per slot (Hawkins/Numenta; Olshausen & Field 1996) | #519 domain-routed sparse panel; #389 OOD |
| **#544 / TASK-PY-NEURO-007** | Synchrony-based temporal binding for #392 (Singer & Engel; Fries 2005) | #392 temporal; #390 constellation intelligence |
| **#545 / TASK-PY-NEURO-008** | Idle-time generative rollouts (default-mode network analog) | #416, #517, #406, #520 |

**Doctrinal compatibility check (all 8 verified before filing):**

- ✅ Slot identity invariant preserved (no cross-embedder dim compare)
- ✅ Anti-compensation invariant (#394) preserved (strong slots cannot silence weak)
- ✅ Binary contract preserved (final verdict still Q2 Pass/Fail; Q3 explanation-only; Q4 frozen)
- ✅ Ambiguity boundary preserved (no ambiguous heads introduced)
- ✅ No inner-LLM assumption introduced
- ✅ No synthetic-input shortcut (all extensions ride real instrument substrate)

---

## 7. EK series (Explanatory Knowledge programme #321) — open tail

| Issue | Status |
|---|---|
| #325 Mechanism-conjecture surface | open p2 |
| #326 `mejepa_counterfactual_under_invariant` MCP tool | open p2 |
| #328 Held-out failure-mode class falsification | open p2 |
| #329 AlphaEvolve-pattern outer loop using `RealityPrediction` as fitness | open p2 |
| #330 Ontology-growth audit + telemetry | open p2 |
| #335 `mejepa_reverse_prediction` — desired-reality → patch-shape constraints | open p2 |
| #336 Operator-contribution surface | in-progress |
| #339 Chunk foundationality | in-progress |
| #340 Library-level foundationality | in-progress |
| #349 Embedder-foundationality | in-progress |
| #352 Credibility-weighted operator override | in-progress |
| #354-#357 v2 stretch (NAS embedder / cross-library / self-rewriting / status reallocation) | p3 |

---

## 8. Aiwonder migration (open tail)

**Major milestone (2026-05-22):** **#372 CLOSED** — ContextGraph roots mounted on aiwonder ZFS pools. Legacy mounted-drive paths (`/mnt/d`, `/mnt/c`) forbidden in active code/FSV.

| Issue | Task | Status |
|---|---|---|
| #310 | META Issue refactor | in-progress p0 |
| #372 | ContextGraph roots must be mounted on aiwonder ZFS pools | ✅ closed |
| #299-#305 | AIWONDER-003..010 install / transfer / build / launch / calibration | all closed with #372 |
| #295 | PERF — resume launch scans full aiwonder FSV root | in-progress |
| #368 | Host capability utilization audit + scheduler profile | blocked p3 (behind #633/#634) |
| #380 | Off-host backup for `/zfs/archive` proof history | blocked p3 |
| #384 | CUDA/candle rollback and reproducibility lane | blocked p3 |
| #306 | SWE-bench Lite Python Docker image plan (future / Q3) | blocked p3 |
| #312 | hot_root on ext4 throughput bottleneck — superseded by #372 | satisfied |
| #442 | Track aiwonder librocksdb-sys cstdint native build requirement | blocked p1 |
| #617 | WSL `/mnt/c` capacity snapshots remain in aiwonder FSV scripts | needs-triage p2 |
| #671 | Stale SWE-bench harness defaults point at missing aiwonder archive venv | ✅ closed |
| #674 | `run-remote` wrapper can hang after remote command completes | ✅ closed |

---

## 9. Active `type:context` pins (read at session start)

| # | Title | Why pinned |
|---|---|---|
| **#712** | **Sherlock-validated critical path** | **Authoritative ordering.** New ranks supersede prior critical-path orderings; post-#704/#714 active substrate work is #709/#715. |
| #633 | [P0] Manual full-system FSV and hardening gate | Active in parallel (rank 7 of #712); children #634-#651 still required |
| #310 | META Issue refactor | Phase G execution-order map; now subordinate to #712 |
| #383 | Gate-active issue order must be evidence-weighted (p0) | Evidence per #712 |
| #397 | ME-JEPA consequence-prediction truth-system contract (p0) | Binary contract |
| #227 | Python ME-JEPA data explosion runtime policy | Forward-cache job constraints |

---

## 10. Bugs (open)

### 10.1 P0 bugs

| # | Title |
|---|---|
| #618 | Aggregate OOD score uses flat 5,120-byte panel buffer (doctrine §1.5ter) |
| #619 | `forward_inverse_inner` uses `Linear(PANEL_DIM, hidden)` flat projections (doctrine §1.5ter) |
| #621 | Hardcoded `gtau_pass_rate: 1.0` published to MCP weekly dashboard |
| #685 | `AuxHeadEnsemble::forward_logits` returns zeros regardless of input |
| #687 | `detach_loras` is a public no-op that always returns Ok |
| #688 | `EpochSummary` hardcodes best/worst category and language |
| #690 | `mejepa-heal-drill` hardcodes `retrain_triggered: true` |
| #695 | Rank-lift exemplar transformation is a Goodhart magnifier, not a calibration |
| #699 | `compute_train_health` reads pseudo-training certificates |
| #709 | Add inter-embedder distinctness gate before cache promotion |
| #715 | Rebuild static AST forward cache with 9-slot policy and post-fix E8 |
| #656 | mejepa-heal-drill fails before complete readback on conformal overcoverage (in-progress) |
| #616 | Rerun and classify weak-cell oracle timeout/error rows |
| #620 | `MEJEPA_INSTRUMENT_GRADIENT_LEAK` runtime hook never invoked from training |
| #622 | Trainer dead-code cleanup batch (distill + lora_wiring + aux) |

### 10.2 Additional bug queue

| # | Title | Status |
|---|---|---|
| #623 | `cold_start_conformal_policy` invalid `alpha=0.0` dead public API | needs-triage p3 |
| #624 | `verdict_assembly` inline `CONTRADICTION_CALIBRATION_INVALID` not declared constant | needs-triage p3 |
| #625 | `DeterministicPredictor` publicly re-exported | needs-triage p2 |
| #626 | Hygiene retention default policy missing ~90 CFs (`config/retention_policy.toml` stale) | needs-triage p1 |
| #627 | shift-subscriber runner integration test fails on main | needs-triage p2 |
| #628 | Doc/code error-string drift: `MEJEPA_TCT_DIM_MISMATCH` vs `MEJEPA_INFER_DIM_MISMATCH` | needs-triage p3 |
| #630 | Recover long-source/OOM forward-cache quarantines after legal mask | needs-triage p1 |
| #663 | Resolve duplicate TASK-PY-G keys after geometry-label issue creation | needs-triage p2 |
| #693 | Phase H absence-check script/doc drift (Qwen/Hermes/Ollama/llama.cpp) | needs-triage p1 |
| #307 | promote_tree_verified asserts on second-subrun shard promotion | in-progress |
| #320 | phase_g_project_merkle_cache_fsv clippy — `ProjectMerkleCache` Debug missing | needs-triage p1 |
| #338 | Capture agent response bodies in runtime/agent-history for Q1 claim FSV | blocked p3 |
| #429 | DynamicJEPA Phase 8 FSV intermittently aborts in native record-surprise path | blocked p3 |
| #433 | P1 ability resolver evidence integrity and replay key completeness | blocked p3 |
| #436 | context-graph-mejepa example clippy gate has pre-existing warnings | blocked p3 |
| #453 | Residual Sherlock/investigation findings after first cleanup pass | needs-triage p1 |
| #522 | verification_state test: predicted SubtleFlip failure no longer escalates | needs-triage p1 |

---

## 11. Deferred (not blocking ship gate)

| # | Why deferred |
|---|---|
| #149 | SWE-bench Verified subset — separate dataset; ship gate uses Lite 300×8 |
| #354-#357 | TASK-EK-018 v2 stretch (NAS from scratch / cross-library / self-rewriting / status reallocation) |
| Non-Python ship-gate cells (Rust/JS/TS/Go/Java/C/Cpp/CSharp/Ruby/Php) | Active execution Python-first; non-Python deferred until Python truth system green |

---

## 12. Phase G calibration sweep (open p1/p2)

All items below are qualified by #385: evidence must be per-slot/per-embedder constellation, not flat panel aggregate.

| Issue | Task | Status |
|---|---|---|
| #122 | refined 66-cell pairwise MI matrix audit | open |
| #123 | per-cell TCT signal-saturation matrix | open |
| #124 | frozen instrument coverage audit per Python cell | open |
| #125 | Q1 claim-existence calibration at corpus scale | open |
| #126 | Q3 failure-mode calibration | open |
| #121 | Conformal RCA classifier | open |
| #152 | E_Trace gating | open |

Gate: per-head Brier ≤ 0.05, ECE ≤ 0.05, conformal coverage 88-92%.

---

## 13. Continuous self-improvement + agent feedback loop (open p1)

| Issue | Task | Status |
|---|---|---|
| #154 | Active-learning consumer (multi-source queue) | open |
| #131 | Feedback cooldown (prevent reward thrashing) | open |
| #163 | Feedback retrain consumer | open |
| #182 | Feedback replay-weight FSV | open |

---

## 14. Adversarial defense + ship-gate closure (blocked behind #633)

| Issue | Task | Status |
|---|---|---|
| #120 | Catastrophic Python cell quarantine | blocked p3 |
| #148 | Python leakage detector | blocked p3 |
| #153 | Python oracle pass/fail audit | blocked p3 |
| #165 | Python 0.95 confidence gate | blocked behind #167 |
| #167 | Four-window Python burn-in | blocked behind #633 + #610 |
| #172 | Python acceptance bundle | blocked p3 |
| #180 | MCP `RealityPrediction` FSV | blocked p3 |
| #171 | Fail-closed release switch | blocked p3 |
| #379 | Multi-file structural patch holdout | blocked p3 |
| #382 | Frozen-panel dimensionality ablation | blocked p3 |
| #375 | Consume witness provenance in `RealityPrediction.confidence` | blocked p3 |
| #378 | MCP tool-usage telemetry + dead-surface audit | blocked p3 |

---

## 15. Post-ship operation (engages after `≥ 0.95 × 4 windows`)

| Issue | Task |
|---|---|
| #277 | Post-ship-gate auto-trust policy |
| #173 | Post-green drift guardrails |
| #170 | Model-quality RCA dashboard |
| #168 | Live Python patch backfill |
| #164 | Checkpoint provenance |
| #162 | Eval-after-promotion hook |
| #166 | Python error taxonomy |
| #169 | Unseen Python holdout quarantine |

---

## 16. How to pick up work (per AGENTS.md)

1. Run the six-issue queries from `AGENTS.md`:
   ```bash
   gh issue list --state open --label "type:context" --json number,title,body
   gh issue list --state open --label "status:in-progress" --json number,title,assignees
   gh issue list --state open --label "status:blocked" --json number,title
   gh issue list --state open --label "source:agent" --search "no:assignee" --json number,title,labels,updatedAt
   gh issue list --state closed --label "type:decision" --limit 20
   gh issue list --state closed --label "type:discovery,type:pattern" --search "<kw>" --limit 20
   ```
2. Read `README.md` for the current doc map, then `01 §1-2`, `07 §1`, this roadmap.
3. Find the highest-priority open issue you can usefully unblock that does NOT have an in-progress assignee.
4. CLAIM (assign self + flip `status:needs-triage` → `status:in-progress` + post CLAIM comment with files-you-will-touch + ETA).
5. Audit-the-task before writing code (`06 §1.6`).
6. Work; comment at every milestone.
7. Produce FSV evidence per `06 §1.5` and manual SoT readback per `06 §1.9`.
8. RESOLVED / PAUSE / BLOCKED issue closeout.
9. Code-simplifier pass.
10. File any observed defect / pattern / discovery as a fresh issue before ending turn.

---

## 17. What this doc does NOT cover

- Day-to-day operator decisions outside the issue tracker — those happen in conversation with the operator and become `type:decision` issues when locked in.
- Hardware procurement / aiwonder admin — `phase:migration` issues track that subset.
- Time estimates — Phase G has no public ETA; per-cell convergence ETA tracker emits internal projections used by the heal scheduler.
