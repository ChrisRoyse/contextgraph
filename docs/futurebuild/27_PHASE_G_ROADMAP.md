# 27 — Phase G Roadmap: The Reality Compiler Critical Path

> **Purpose.** Single linear roadmap an AI agent picks up to drive ME-JEPA-Code to the ship gate (`prediction_oracle_correlation ≥ 0.95` stable × 4 rolling windows on the 300×8 Python SWE-bench Lite corpus) **and** to the operational "reality compiler" state where it can ingest an arbitrary user Python codebase, predict reality, learn continuously from feedback, and improve itself the more it is used.
>
> **How to read this file.** Phases are sequential — earlier phases unblock later ones. Within a phase, issues are listed in execution order with explicit dependencies. Every issue has an FSV binding (see §FSV-PROTOCOL). When a Phase is fully resolved, the agent advances to the next.
>
> **Authoring date:** 2026-05-15. Living doc — re-pin on major status change.

---

## §0 — The Operator Goals This Roadmap Serves

1. **Reality prediction** — Given a code change + context, ME-JEPA returns a calibrated `RealityPrediction` (Q1-Q5: existence / works / why / what-else / impact) so AI-generated patches don't need human review.
2. **Reality compiler** — Operator points ME-JEPA at any Python codebase. The system chunks, embeds, materializes panels, computes DDA/TCT, and emits a per-project reality report. Subsequent compiles only re-process changed files (Merkle-tree cache).
3. **Continuous self-improvement** — Every Edit, every test outcome, every operator override updates the predictor online. The more the system is used, the better its predictions become. Catastrophic forgetting is prevented (EWC); plasticity loss is prevented (continual backprop); novel patterns extend the constellation.
4. **Tight AI-agent feedback loop** — Claude Code hooks (PostToolUse, Stop, UserPromptSubmit) capture every test outcome, every claim, every verdict-confirmation/refutation; ME-JEPA's accuracy on past predictions is itself observable.
5. **Adversarial robustness** — The predictor cannot be gamed into emitting a confident `Pass` on a broken patch. Negative-action ablation, multi-head contradiction, OOD gating, and conformal interval floors all stand structurally outside the predictor's loss.
6. **Full state verification end-to-end** — Every primitive resolves by SoT readback against synthetic inputs with known expected outputs, manual verification of physical persistence, and ≥3 edge cases with before/after evidence.

---

## §1 — Phase 1: Data Substrate (in flight)

The reward field must exist before the predictor can be trained.

| Order | Issue | Title | Status | Blocking |
|---|---|---|---|---|
| 1.1 | **#199** | TASK-PY-DATA-016 — Corpus-wide embedder explosion | **in-progress (~0.82%, 24/2933 shards)** | All downstream P2-P8 work |
| 1.2 | **#208** | TASK-PY-DATA-025 — Full data-explosion FSV harness | blocked on #199 | P3 calibration |
| 1.3 | **#236** | TASK-PY-DATA-045 — CUDA 13.2 embedder eligibility audit | blocked on #199 telemetry | GPU correctness |
| 1.4 | **#237** | TASK-PY-DATA-046 — Cross-embedder alignment matrix | blocked on #199 | #238, #247 |
| 1.5 | **#238** | TASK-PY-DATA-047 — Decompose embedder redundancy + synergy | blocked on #237 | #247 |
| 1.6 | **#239** | TASK-PY-DATA-048 — Mine embedder disagreement constellations | blocked on #199, #237/#238 | #247 |
| 1.7 | **#247** | TASK-PY-DATA-056 — Adaptive embedder weighting | blocked on #237-#239 | P2 head training |

**Gate for Phase 1:** All forward-cache rows present in `/mnt/d/contextgraph/corpus/python-forward-cache-v1-task-py-data-016-full-20260514`; per-cell DDA signals persisted in `CF_MEJEPA_DDA_SIGNALS`.

---

## §2 — Phase 2: Q1-Q5 Predictor Producers (schema fields with no builder)

`RealityPrediction` schema (`15_PREDICTION_TAXONOMY.md §1`) has fields with no producer until these ship.

| Order | Issue | Producer for | Notes |
|---|---|---|---|
| 2.1 | **#260** | Q1 `claim_graph` + `claim_reconciliation` | Deterministic claim parser; reconcile to bytes-on-disk at after_sha |
| 2.2 | **#261** | Q3 `closest_exemplars` | Patch-similarity HNSW index over corpus |
| 2.3 | **#262** | All heads — `per_head_feature_projection` | Wires `04_DDA_TCT §3` head→signal table into trained heads |
| 2.4 | **#264** | Q5 `reality_impact` | Shift-log replay engine |
| 2.5 | **#265** | Q4 `predicted_security_concerns` | bandit + semgrep label pipeline |
| 2.6 | **#266** | Q4 `predicted_perf_regressions` | pytest-benchmark + cProfile label pipeline |
| 2.7 | **#267** | Q4 `predicted_accuracy_degradations` | ML test-output metric pipeline |
| 2.8 | **#268** | Q4 `predicted_cost_regressions` | CI minutes + dep counter pipeline |
| 2.9 | **#269** | `predicted_reasoning_class` | Reasoning head training |
| 2.10 | **#118** | All Q1-Q5 heads | Per-head distillation training step |

**Gate for Phase 2:** Every `RealityPrediction` field has a producer that emits structured output on synthetic inputs.

---

## §3 — Phase 3: Predictor Calibration + Density

Calibrators ride on top of producers; reward-signal density audits ensure no cell is starving.

| Order | Issue | Purpose |
|---|---|---|
| 3.1 | **refined #122** | 66-cell pairwise MI matrix audit; redundancy/degeneracy flags |
| 3.2 | **refined #123** | Per-cell TCT signal-saturation matrix (`realized/theoretical`) |
| 3.3 | **#124** | Frozen instrument coverage audit per Python cell |
| 3.4 | **#271** | Per-cell constellation freshness audit + scheduler integration |
| 3.5 | **#125** | Q1 claim-existence calibration at corpus scale (consumes #260) |
| 3.6 | **#126** | Q3 failure-mode calibration (consumes #261 exemplars + #265-#268 labels) |
| 3.7 | **refined #127** | Q4 confidence calibration per class with Brier/ECE/τ |
| 3.8 | **refined #128** | Q5 head calibration vs shift-log replay (#264) |
| 3.9 | **#270** | Q4 non-trivial-on-Pass calibration (always-on validation) |
| 3.10 | **#263** | Multi-head contradiction calibration per cell |
| 3.11 | **#121** | Conformal RCA classifier |
| 3.12 | **#130** | Gτ guard audit |
| 3.13 | **#152** | E_Trace gating |
| 3.14 | **#280** | Cold-start verdict policy (Abstain when n_supporting<50) |
| 3.15 | **#293** | Static-analysis (mypy + pyright + ruff) per-chunk reward signal |
| 3.16 | **#294** | Composite reward signals: coverage + git churn + complexity + doc-coverage + dep-CVE |

**Gate for Phase 3:** Per-head Brier ≤ 0.05, ECE ≤ 0.05, conformal coverage 88-92%.

---

## §4 — Phase 4: Reality Compiler (NEW — usability for arbitrary user codebases)

This is the operator-facing capability: point ME-JEPA at any Python repo and it predicts reality. Subsequent compiles only re-embed changed files.

| Order | Issue | Capability |
|---|---|---|
| 4.1 | **#281** | Merkle-tree per-project chunk/embed cache + file watcher + transitive-invalidation + schema-version-aware re-embed |
| 4.2 | **#282** | Project-ingestion MCP tool with source/test scoping |
| 4.3 | **#283** | Per-project reality compile report (top-N likely-broken areas, edge cases, dep risks) |

**Gate for Phase 4:** Fresh ingest of a 50-file Python project completes < 60 s on RTX 5090; re-ingest after a 1-file edit < 5 s; report renders end-to-end.

External references: Cursor's Merkle-tree codebase indexing; Lumen's per-project content-hashed cache; CKB's transitive invalidation modes; Codemogger's chunk-keyed embedding cache with `embedding_model` invalidation.

---

## §5 — Phase 5: Continuous Self-Improvement (NEW — "learns the more it's used")

Per UTML v1.0 (`docs2/TheoryOfMachineLearning.md`): `L_step = ΔP × ΔK × ΔΩ × ΔΞ`. The system must protect ΔΩ (plasticity), ΔK (catastrophic-forgetting), and the M(t) consolidation integral.

| Order | Issue | Capability | UTML term |
|---|---|---|---|
| 5.1 | **#272** | Online observe loop reward signals | M(t) feedback |
| 5.2 | **#273** | Negative-action ablation falsifier | ΔΞ collapse detection |
| 5.3 | **#284** | EWC catastrophic-forgetting protection | ΔK.fisher_violation |
| 5.4 | **#285** | Continual backprop (Dohare 2024) plasticity prevention | ΔΩ.effective_plasticity |
| 5.5 | **#286** | Novel-pattern detection + automatic constellation cell extension | learn unseen situations |
| 5.6 | **#287** | Replay buffer prioritized by surprise + per-cell coverage gap | M(t) k_replay multiplier |

**Gate for Phase 5:** Over a 4-week burn-in, predictor improves ≥ 3% in per-cell correlation without regressing any cell by > 1%.

---

## §6 — Phase 6: Claude Code Feedback Loop (the agent-side closed loop)

Hook-based capture so every test outcome and agent claim becomes ME-JEPA training pressure.

| Order | Issue | Hook event | Capability |
|---|---|---|---|
| 6.1 | **#274** | UserPromptSubmit | Inject latest RealityPrediction into agent context |
| 6.2 | **#288** | PostToolUse (Bash matcher) | Capture pytest/cargo-test stdout, bind to current prediction_id, write to `CF_MEJEPA_PREDICTION_VERIFICATIONS` |
| 6.3 | **#289** | Stop | Block agent stop until pending predictions are verified-confirmed or verified-rejected (exit code 2 gate) |
| 6.4 | **refined #154** | Active-learning consumer | Multi-source queue: surprise / contradiction / OOD / agent / operator |
| 6.5 | **#275** | End-to-end | Live Claude Code session integration FSV |
| 6.6 | **#276** | OOD | Live-session OOD corpus harvest |
| 6.7 | **#131** | Cooldown | Feedback cooldown (prevent reward thrashing) |
| 6.8 | **#163** | Retrain | Feedback retrain consumer |
| 6.9 | **#182** | Replay-weight | Feedback replay-weight FSV |

**Gate for Phase 6:** One full Claude Code session on a real SWE-bench instance produces ≥ 1 verdict per Edit; PostToolUse capture binds ≥ 90% of test outcomes to a prediction.

External references: Claude Code PostToolUse Bash matcher with `CLAUDE_TOOL_OUTPUT`; Stop hook self-verification loop pattern (ShipWithAI 2026-04-16); UserPromptSubmit injection idiom.

---

## §7 — Phase 7: Adversarial Defense + Ship Gate

| Order | Issue | Purpose |
|---|---|---|
| 7.1 | **#278** | Secret redaction before embedding |
| 7.2 | **#279** | 30-case adversarial Python corpus (10 subtle-bug-passes-tests + 10 KnownGood-lookalike + 10 embedder-gaming) |
| 7.3 | **#290** | Synthetic-Python stress test (operator-authored programs with known expected verdicts) |
| 7.4 | **#291** | End-to-end stress harness over synthetic projects of varying complexity |
| 7.5 | **#292** | Predicted-Works head + test-coverage + test-flakiness sub-heads (explicit positive assertions with confidence + citations) |
| 7.6 | **refined #178** | Cell taxonomy with Active/Exempt/Uncovered tri-state |
| 7.7 | **refined #129** | OOD detection gate calibrated on harvested + synthetic OOD |
| 7.8 | **#120** | Catastrophic Python cell quarantine |
| 7.9 | **#148** | Python leakage detector |
| 7.10 | **#153** | Python oracle pass/fail audit |
| 7.11 | **#165** | Python 0.95 confidence gate |
| 7.12 | **#167** | Four-window Python burn-in |
| 7.13 | **#172** | Python acceptance bundle |
| 7.14 | **#180** | MCP RealityPrediction FSV |
| 7.15 | **#171** | Fail-closed release switch |

**Gate for Phase 7 (SHIP):** `prediction_oracle_correlation ≥ 0.95` stable × 4 rolling windows × all Python mutation cells. Adversarial corpus achieves 0 `Pass@conf>0.95` on lookalikes.

---

## §8 — Phase 8: Post-Ship Operation

| Order | Issue | Purpose |
|---|---|---|
| 8.1 | **#277** | Post-ship-gate auto-trust policy |
| 8.2 | **#173** | Post-green drift guardrails |
| 8.3 | **#170** | Model-quality RCA dashboard |
| 8.4 | **#168** | Live Python patch backfill |
| 8.5 | **#164** | Checkpoint provenance |
| 8.6 | **#162** | Eval-after-promotion hook |
| 8.7 | **#166** | Python error taxonomy |

---

## §9 — Deferred / Out-of-Critical-Path

These remain open but are not blocking the ship gate. Pick up only after Phase 7 closes.

| Issue | Why deferred |
|---|---|
| **#149** | SWE-bench Verified subset — separate dataset; ship gate uses Lite 300×8 (priority dropped p2→p3 2026-05-15) |
| **#150** | Python sample-efficiency analysis — diagnostic; not load-bearing for gate (priority dropped p2→p3 2026-05-15) |
| **#119** | Python generalization audit — overlaps with #169 (unseen holdout) |
| **#169** | Unseen Python holdout quarantine — fold into Phase 7 if cleared by #148 |
| **#256** | Bug: dynamicjepa transfer FSV aborts — proof-domain, retired path |
| **#151** | CLOSED 2026-05-15 — subsumed by #273 (negative-action ablation falsifier) |

---

## §FSV-PROTOCOL — Mandatory for Resolution (every issue in this roadmap)

Resolution of any issue in this roadmap requires **Full State Verification**. A return value is a claim; the Source of Truth is the verdict; read the verdict.

1. **Define the Source of Truth** — name the exact RocksDB CF + key / file path + sha / JSONL row / counter / MCP read-side tool. State the expected value before running.
2. **Execute and Inspect** — run the logic, then perform a **separate `Read` operation** on the SoT. Re-open the CF read-only, re-decode bytes through code that is not the writer, re-hash the file, query the database independently.
3. **Manual physical verification** — if output lands in a DB/table/graph/file, open it and look. If it's a counter, read the metric. If it's an MCP response, call the read-side tool from a fresh process.
4. **Synthetic inputs with known expected outputs** — construct ≥ 3 synthetic inputs where you know the answer ahead of time ("2+2=4" pattern). Run them through the pipeline. Log the SoT state before and after.
5. **Boundary / edge-case audit** — manually simulate ≥ 3 edge cases: empty input, maximum-limit / over-limit input, malformed-format input. For each, print before/after SoT state.
6. **Trigger → Process → Outcome chain** — state the trigger event, the process X it initiates, the intended outcome Y. Verify Y at the SoT.
7. **Root-cause discipline** — on any test failure or unexpected behavior, stop and identify the actual root cause via first-principles decomposition. Do not paper over the symptom.
8. **Best-practices research** — web-research best practices; cite ≥ 2 external sources in the RESOLVED comment.
9. **Code-simplifier review** — invoke the code-simplifier subagent after implementation; apply refinements; record them.
10. **Evidence of success** — provide a log showing actual data residing in the system: byte hashes, line counts, row counts, CF size, re-open readback values.

**Definition of Done (FSV-binding):**
- [ ] Source of Truth named (exact path/CF/key).
- [ ] Happy-path synthetic run with known expected output: PASS.
- [ ] ≥ 3 edge cases with before/after SoT readback: PASS.
- [ ] Independent re-open / re-read confirms persisted state.
- [ ] Byte-equal-readback for RocksDB writes (sync_all + reopen + decode).
- [ ] Web-research best-practices cited (≥ 2 sources).
- [ ] Code-simplifier review completed; refinements applied.
- [ ] FSV evidence file at `/mnt/d/contextgraph/fsv/<task-slug>-fsv/<task>-fsv.json` with `passes=true`.
- [ ] RESOLVED issue comment with commit SHA, exact commands, SoT readback values, edge-case evidence, residual risk.

**Fail-closed on unknowns.** If any of the four FSV questions ("what state changed / where does it live / how do I read it back independently / what is the expected value") cannot be answered, the feature is not understood well enough to call done. Emit a typed `SCREAMING_SNAKE_CASE` error and surface the gap.

**No mocks** in tests that gate task resolution. **No tests that pass when the project is broken.** **No silent fallbacks** for required outputs.

---

## §10 — How an AI agent should use this roadmap

1. **Read this file top-to-bottom once per session.** It is the single source of plan-of-record.
2. **Find the lowest-numbered phase with any incomplete issue.** Within that phase, find the lowest-ordered incomplete issue not blocked by an in-progress sibling.
3. **Claim the issue per CLAUDE.md §9** — flip `status:needs-triage → status:in-progress`, post a CLAIM comment.
4. **Read the issue body, follow the FSV protocol exactly.** Define SoT before running anything.
5. **At completion, post RESOLVED comment with the FSV evidence + commit SHA + log of SoT readback values.**
6. **Update this file** if the phase boundary moves or if a new issue is filed that fits an existing phase. Re-pin when major status changes.

---

## §11 — Living references

- `CLAUDE.md` — project addendum, doctrine, FSV contract reference.
- `docs2/AICodingAgentSuperPrompt.md` — universal doctrine.
- `docs2/TheoryOfMachineLearning.md` — UTML v1.0 four-factor framework grounding ME-JEPA's collapse-immunity and consolidation claims.
- `docs/futurebuild/01_VISION_TRUTH_SYSTEM.md` — Q1-Q5 vision.
- `docs/futurebuild/04_DDA_TCT_CODE_ENRICHMENT.md` — per-head DDA signal table.
- `docs/futurebuild/15_PREDICTION_TAXONOMY.md` — `RealityPrediction` schema.
- `docs/futurebuild/22_ADVERSARIAL_ROBUSTNESS.md` — defense layer.
