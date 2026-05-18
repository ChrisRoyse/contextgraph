# 31 — Complete Roadmap: Every Task to Get from Today to Self-Designing ME-JEPA

> **Authored:** 2026-05-17 in response to operator directive *"create the entire roadmap, lay out the whole thing in docs and in the github issues. don't leave anything out. every task that needs completed must be listed."*
>
> **Status:** living document. This is the single canonical reference for *every* open task across Phase G (current 0.95 ship gate), Phase H (residue cleanup, already closed), Phase I (EPIC #321 explanatory-knowledge programme), and Phase J (post-018 generalisation). When this doc and any other futurebuild doc disagree, this one wins for the *task inventory*; the other doc wins for the *spec detail*.

---

## §0 — Reading order

This roadmap is organised by **phase** then by **deliverable**. Each deliverable lists its atomic GitHub issues with one-line status. Issue numbers are GitHub-permanent; task keys (TASK-PY-G-*, TASK-FP-*, TASK-G-*, TASK-RWD-*, TASK-EK-*) are doctrine-permanent.

| Section | Phase | Coverage |
|---|---|---|
| §1 | Phase G — current | Close the 0.95 Python ship gate. ~50 atomic issues, mostly shipping or shipped. |
| §2 | Phase H — done | Inner-loop residue cleanup. All issues closed. Listed for completeness. |
| §3 | Phase I — EPIC #321 | Explanatory-knowledge programme. Eighteen TASK-EK-* tasks + TASK-EK-018 apex deliverable decomposed into 10 atomic v1 sub-issues + 4 v2 stub issues. |
| §4 | Phase J — generalisation | Post-018 multi-language, multi-domain (ME-JEPA-X) expansion. |
| §5 | Cross-cutting hardening | DR, runbook, security, multi-agent isolation, API stability. Spans all phases. |
| §6 | Priority queue | The recommended execution order across all phases. |

---

## §1 — Phase G: Close the 0.95 Python ship gate

**The only outstanding gate that blocks human-review-replacement.** Per the latest verified ship-gate FSV: `prediction_oracle_correlation = 0.866667 < 0.95`. Every Phase G issue below either directly contributes to closing this gate or is operating-cadence infrastructure that the gate-closure depends on.

### §1.1 — Data substrate (Phase 1 of `27_PHASE_G_ROADMAP.md`)

| # | Task | Issue | Status |
|---|---|---|---|
| 1.1 | Corpus-wide embedder explosion + bounded resume | #199 / #295 | in-flight on aiwonder; substrate complete at `/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/` |
| 1.2 | Full data-explosion FSV harness | #208 | pending |
| 1.3 | CUDA 13.2 embedder eligibility audit | #236 | pending |
| 1.4 | Cross-embedder representation alignment matrix | #237 | pending |
| 1.5 | Decompose embedder redundancy + synergy | #238 | pending |
| 1.6 | Mine embedder disagreement constellations | #239 | pending |
| 1.7 | Adaptive embedder weighting | #247 | pending |
| 1.8 | Python embedder MI rotation (now EXP-2 substrate for TASK-EK-003) | #122 | refactored 2026-05-17 |

### §1.2 — Q1–Q5 predictor producers (Phase 2 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 2.1 | Q1 claim_graph + claim_reconciliation | #260 | pending |
| 2.2 | Q3 closest_exemplars (patch-similarity HNSW) | #261 | pending |
| 2.3 | All heads: per-head DDA feature projection | #262 | pending |
| 2.4 | Q5 reality_impact (shift-log replay) | #264 | pending |
| 2.5 | Q4 security analyzer label pipeline | #265 | TASK-RWD-204 shipped 2026-05-17 |
| 2.6 | Q4 perf analyzer label pipeline | #266 | pending |
| 2.7 | Q4 accuracy analyzer label pipeline | #267 | pending |
| 2.8 | Q4 cost analyzer label pipeline | #268 | pending |
| 2.9 | Predicted reasoning class | #269 | pending |
| 2.10 | Per-head Q1-Q5 distillation | #118 | pending |
| 2.11 | Q4 non-trivial-on-Pass calibration | #270 | pending |
| 2.12 | Predicted-Works head + test-coverage sub-heads | #292 | pending |

### §1.3 — Calibration + density (Phase 3 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 3.1 | Per-cell constellation freshness audit + scheduler | #271 | pending |
| 3.2 | Multi-head contradiction calibration | #263 | pending |
| 3.3 | Online observe loop reward signals | #272 | pending |
| 3.4 | Negative-action ablation falsifier | #273 | pending |
| 3.5 | Q5 impact vs shift-log | #128 | pending |
| 3.6 | Q4 concern confidence calibration | #127 | pending |
| 3.7 | Q3 failure-mode calibration | #126 | pending |
| 3.8 | Q1 claim existence at corpus scale | #125 | pending |
| 3.9 | Frozen instrument coverage audit | #124 | pending |
| 3.10 | Python TCT signal-density audit | #123 | pending |
| 3.11 | Python conformal RCA classifier | #121 | pending |
| 3.12 | Python Gτ guard audit | #130 | pending |
| 3.13 | Python OOD detection gate | #129 | pending |
| 3.14 | Python E_Trace gating | #152 | pending |
| 3.15 | Static-analysis reward signal (mypy + pyright + ruff) | #293 | TASK-RWD-208 shipped 2026-05-17 |
| 3.16 | Coverage + churn + complexity + doc + dep-CVE reward | #294 | TASK-RWD-209 shipped 2026-05-17 |
| 3.17 | Cold-start verdict policy | #280 | TASK-PY-G-061 shipped |

### §1.4 — Reality compiler usability (Phase 4 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 4.1 | Merkle-tree per-project chunk/embed cache | #281 | shipped 2026-05-18 |
| 4.2 | Project-ingestion MCP tool | #282 | shipped 2026-05-18 — directory/direct-file source paths, MCP canonical+alias dispatch, physical CF key readback, FSV sha256 `d3b8bc988498976b0db0dd16dd1024250ac0428935c28cdc2b63fe4962a6408b` |
| 4.3 | Per-project reality compile report | #283 | shipped 2026-05-18 - report.json/report.md, CF_MEJEPA_PROJECT_REPORTS, MCP canonical+alias, FSV sha256 `853fe46e9d921f78cad9ba8fa3ec0e7cc07dc4b104d1bfd5bb0144ea00c739e4` |

### §1.5 — Continuous self-improvement (Phase 5 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 5.1 | EWC catastrophic-forgetting protection | #284 | shipped 2026-05-18 - predictor-wide EWC gradient, per-cell lambda decay, Fisher-budget violation gate, active-learning requeue, `ewc_violations_total`, 1k-update Fisher snapshots, FSV sha256 `7c9cd8fc37889c4176fd590ae4f2182b2d6abaf83386b474199b68629c8af4d4` |
| 5.2 | Continual backprop plasticity prevention | #285 | pending |
| 5.3 | Novel-pattern detection + auto constellation extension | #286 | refactored 2026-05-17 as TASK-EK-002 prerequisite |
| 5.4 | Prioritised replay buffer | #287 | pending |
| 5.5 | Active-learning consumer | #154 | pending |
| 5.6 | Feedback cooldown | #131 | pending |
| 5.7 | Feedback retrain consumer | #163 | pending |
| 5.8 | Feedback replay-weight FSV | #182 | pending |

### §1.6 — Claude Code feedback loop (Phase 6 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 6.1 | Live RealityPrediction prompt injection | #274 | shipped 2026-05-17 (commit `693a0ebe`) |
| 6.2 | PostToolUse Bash test-result capture | #288 | shipped 2026-05-17 (commit `22646deb`) |
| 6.3 | Stop hook ME-JEPA self-verification gate | #289 | shipped 2026-05-17 (commit `2759ff3b` + `64ed1106` + `18e7580b` + `6a9016fd`) |
| 6.4 | Live Claude Code session integration FSV | #275 | pending |
| 6.5 | Live-session OOD corpus harvest | #276 | pending |
| 6.6 | Eval-after-promotion hook | #162 | pending |

### §1.7 — Adversarial defense + ship gate (Phase 7 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 7.1 | Secret redaction before embedding | #278 | TASK-PY-G-059 PHASE 1 shipped |
| 7.2 | Adversarial fingerprint integration | #279 / #318 | TASK-FP-009 shipped 2026-05-17 |
| 7.3 | Synthetic-Python operator-authored stress corpus | #290 | pending |
| 7.4 | End-to-end reality-compiler stress harness | #291 | pending |
| 7.5 | Catastrophic Python cell quarantine | #120 | pending |
| 7.6 | Python leakage detector | #148 | pending |
| 7.7 | Python oracle pass/fail audit | #153 | pending |
| 7.8 | Python 0.95 confidence gate | #165 | pending |
| 7.9 | Four-window Python burn-in | #167 | pending |
| 7.10 | Python acceptance bundle | #172 | pending |
| 7.11 | MCP RealityPrediction FSV | #180 | pending |
| 7.12 | Fail-closed release switch | #171 | pending |
| 7.13 | Python unseen holdout quarantine | #169 | pending |
| 7.14 | Python generalization audit | #119 | pending |

### §1.8 — Post-ship operation (Phase 8 of roadmap 27)

| # | Task | Issue | Status |
|---|---|---|---|
| 8.1 | Post-ship-gate auto-trust policy | #277 | pending |
| 8.2 | Post-green drift guardrails | #173 | pending |
| 8.3 | Model-quality RCA dashboard | #170 | pending |
| 8.4 | Live Python patch backfill | #168 | pending |
| 8.5 | Checkpoint provenance | #164 | pending |
| 8.6 | Python error taxonomy | #166 | pending |

### §1.9 — Phase G operations infrastructure (ongoing)

| # | Task | Issue | Status |
|---|---|---|---|
| G.9.1 | Aiwonder ext4 → ZFS throughput move | #312 | pending |
| G.9.2 | SWE-bench Lite Python Docker image plan | #306 | deferred Q3 |
| G.9.3 | META issue refactor | #310 | open; periodically updated |

---

## §2 — Phase H: Inner-loop residue cleanup (DONE)

All Phase H issues are closed. Listed for completeness:

- #132 SMAP superseded decisions
- #133 retired MCP tools
- #134 inner-actor model files absence audit
- #135 stale reality-loop binary delete
- #136 retired skills audit
- #137 CLAUDE.md inner-loop reference cleanup
- #138 retired agents marking
- #139 CCREALITY engine retired gate
- #140 scripts/config inner-LLM residue audit

---

## §3 — Phase I: EPIC #321 explanatory-knowledge programme

The Phase-I queue is **the new post-Phase-G work**. None blocks the 0.95 ship gate; all are additive. The single apex deliverable is **TASK-EK-018 / #341** which collapses the prior 17 EK additions into one underlying primitive.

### §3.1 — The eighteen original TASK-EK-* tasks

| # | Task | Issue | Status | Notes |
|---|---|---|---|---|
| EK.001 | Compression-progress telemetry | TASK-EK-001 / #322 | substrate shipped (commit `e6da59e8`) | Schmidhuber-completeness primary metric |
| EK.002 | `mejepa_propose_instrument` | TASK-EK-002 / #323 | subsumed by TASK-EK-018 | becomes proposal-surface |
| EK.003 | `mejepa_mincut_panel` | TASK-EK-003 / #324 | substrate shipped (commit `35b270ec`) | absence-detection substrate |
| EK.004 | Mechanism-conjecture surface | TASK-EK-004 / #325 | pending | Popperian witness loop |
| EK.005 | Counterfactual under invariant | TASK-EK-005 / #326 | pending | Pearl Rung-3 |
| EK.006 | Curiosity reward | TASK-EK-006 / #327 | code/FSV-shipped | `curiosity_score`, ranked queue, telemetry, weekly table; TASK-EK-018 priority queue input |
| EK.007 | Held-out failure-mode class falsification | TASK-EK-007 / #328 | pending | extended to also cover adversarial-embedder-injection per TASK-EK-018J |
| EK.008 | AlphaEvolve bridge | TASK-EK-008 / #329 | pending | pattern reused by TASK-EK-018D for algorithmic-embedder synthesis |
| EK.009 | Ontology-growth audit | TASK-EK-009 / #330 | pending | tracks TASK-EK-018 promotions as ontology growth |
| EK.010 | Retired north-star / intent elicitation surface | TASK-EK-010 / #332 | retired by #358/#359 | not active; do not model, classify, synthesize, infer, or embed private human intention |
| EK.011 | Retired destination-goal synthesis | TASK-EK-011 / #333 | retired by #358/#361 | not active; do not synthesize human goals |
| EK.012 | Bidirectional corpus training | TASK-EK-012 / #334 | code/FSV-shipped, neutral ablation | four-variant 300x8 ablation complete; no measured gain, keep `lambda_inv=0.0` |
| EK.013 | `mejepa_reverse_prediction` | TASK-EK-013 / #335 | pending | inverse verifier for AlphaEvolve priors |
| EK.014 | Operator-contribution surface | TASK-EK-014 / #336 | pending | extended for embedder-review attribution per TASK-EK-018 |
| EK.015 | Credibility-weighted operator override | not yet filed | stretch (file when EK-014 green) | uses EK-014 quality scores in BatchSampler |
| EK.016 | Chunk foundationality scoring | TASK-EK-016 / #339 | pending | substrate for TASK-EK-018F |
| EK.017 | Library-level foundationality | TASK-EK-017 / #340 | pending | per-library deployment target for TASK-EK-018 |
| EK.018 | **Self-designed embedders (apex)** | **TASK-EK-018 / #341** | **pending** | **collapses 002 + 016 + 017 + E11-question + §10.2 ontology framing** |

### §3.2 — TASK-EK-018 v1 atomic sub-issues (filed under #341)

To make TASK-EK-018 implementable as parallel work, decompose into atomic sub-issues:

| # | Sub-task | Issue | Piece | Effort |
|---|---|---|---|---|
| 018A | Dynamic-registration scaffolding (EDynamic enum + CFs + hot-swap) | #342 | Piece 5 | ~2-3 weeks |
| 018B | Absence-detection priority queue (compose existing signals) | #344 | Piece 1 | ~1 week |
| 018C | Falsification gate (per-cell + global, reuse TASK-G-016 promotion gate) | #343 | Piece 4 | ~1-2 weeks |
| 018D | Sub-case (a) algorithmic-embedder synthesis loop | #345 | Piece 2a + Piece 3 ii/iii | ~2-3 weeks |
| 018E | Sub-case (b) learned-head synthesis loop on E7 | #347 | Piece 2b | ~2-3 weeks |
| 018F | First concrete instance: TransE-on-corpus-entity-graph (E11 replacement) | #348 | Piece 3 i | ~1-2 weeks |
| 018G | Freeze + audit machinery (provenance CFs + JEPA invariant check) | #346 | Piece 6 | ~1 week |
| 018H | Operator-in-the-loop gates (first-promotion approval + catastrophic-class) | #350 | §4 | ~1 week |
| 018I | VRAM budget integration for dynamic embedders | #353 | — | ~0.5 week |
| 018J | Adversarial-embedder-injection threat mitigation (extends TASK-EK-007) | #351 | §7 risk 4 | ~1 week |

### §3.3 — TASK-EK-018 v2 stub issues (file as separate epic when v1 is green)

| # | Sub-task | Issue | Effort |
|---|---|---|---|
| 018-v2-A | Sub-case (c) fully neural embedder via NAS | #356 | ~3-6 months |
| 018-v2-B | Cross-library candidate proposals | #357 | ~1-2 months |
| 018-v2-C | Self-rewriting absence-detection priors | #355 | ~2-3 months |
| 018-v2-D | Status-reallocation extension for embedder-review attribution | #354 | ~1 month |

### §3.4 — Embedder-foundationality extension to TASK-EK-016

| # | Sub-task | Issue | Effort |
|---|---|---|---|
| 016-ext | PageRank on embedder-dependency-graph (including TASK-EK-018 dynamic embedders) | #349 | ~1-2 weeks |

### §3.5 — TASK-EK-015 (filed 2026-05-17 as stretch under TASK-EK-014)

| # | Sub-task | Issue | Effort |
|---|---|---|---|
| 015 | Credibility-weighted operator override (BatchSampler weight = 4 + 4 × operator_quality_score) | #352 | ~3-5 days once EK-014 green |

---

## §4 — Phase J: Post-018 generalisation

After TASK-EK-018 ships and the framework can grow its own embedder panel, the next axis is:

| # | Task | Issue | Notes |
|---|---|---|---|
| J.1 | Non-Python language coverage activation (Rust, JS, TS, Go, Java, C, C++, C#, Ruby, PHP) | TBD | substrate (11-language chunker, mutation operators) shipped; ship gate currently Python-only |
| J.2 | Active activation of the 5,120-row non-Python fixture corpus as a holdout | TBD | currently archived future-generalization evidence |
| J.3 | SWE-bench Verified Python subset (separate dataset) | #149 | deferred p3 |
| J.4 | Python sample-efficiency analysis | #150 | deferred p3 |
| J.5 | ME-JEPA-Voice template instantiation per `25_FUTURE_DOMAINS_MEJEPA_X.md` | TBD | reuses TASK-EK-018 for voice-domain embedder self-design |
| J.6 | ME-JEPA-Image template instantiation | TBD | same |
| J.7 | ME-JEPA-X arbitrary-domain template | TBD | the universal-explainer claim taken to its logical conclusion |

---

## §5 — Cross-cutting hardening (spans all phases)

| # | Task | Issue | Phase | Notes |
|---|---|---|---|---|
| X.1 | Operator runbook completeness | (covered in 19) | always | runbook-coverage CI lint shipped |
| X.2 | DR drill cadence | (covered in 20) | always | systemd timer shipped |
| X.3 | Adversarial robustness | (covered in 22 + TASK-EK-018J) | always | TASK-EK-007 + TASK-EK-018J cover the dynamic-embedder extension |
| X.4 | Multi-agent isolation | (covered in 23) | always | session_id isolation shipped; multi-tenant pending |
| X.5 | API stability + semver | (covered in 24) | always | deprecation framework shipped; consumer wiring pending (TASK-API-006) |
| X.6 | Bootstrap + cold start | (covered in 21) | always | cold-start verdict policy shipped (TASK-PY-G-061) |
| X.7 | Storage location policy | (covered in 16) | always | enforced via lint + daemon validator |
| X.8 | Documentation map currency | (covered in 26) | always | this doc (31) is a new entry |

---

## §6 — Priority queue (recommended execution order across all phases)

The single most important ordering question: **what should ship next?**

### §6.1 — Tier 1 (must-ship to close the 0.95 ship gate)

These directly attack the 0.95 metric. Prioritise compute and operator attention here.

1. **§1.3 calibration tasks** (#121, #122, #123, #124, #126, #127, #128, #129, #130) — these are the per-head per-cell calibration tasks that the per-cell ship-gate audit depends on. Effort: ~2–4 weeks each, parallelisable.
2. **TASK-EK-008 / #329** (AlphaEvolve-pattern outer loop) — now higher ROI than #334 because the #334 ablation was neutral.
3. **§1.7 adversarial defense tasks** (#290, #291) — synthetic-Python stress corpus + end-to-end stress harness need to ship before the 0.95 gate can be claimed defensibly.
4. **TASK-EK-014 / #336** (operator-contribution surface) — measure source-backed operator critique/override work without modeling private intention.
5. **TASK-EK-013 / #335** (reverse-prediction patch-shape constraints) — useful for search priors, but must stay constrained to desired observable `RealityPrediction` states and explicit safety constraints.

### §6.2 — Tier 2 (Phase I after gate closes; build the substrate for TASK-EK-018 in parallel)

These do not block the gate but their substrate is needed for TASK-EK-018. Schedule in parallel with Tier 1 when bandwidth allows.

6. **TASK-EK-003 / #324** (mincut panel) — substrate shipped commit `35b270ec`; MCP tool + CLI wiring + FSV pending. Effort: 3–5 days.
7. **TASK-EK-016 / #339** (chunk foundationality scoring) — substrate exists (`crates/context-graph-solver/src/pagerank.rs`); wiring + FSV pending. Effort: 3–5 weeks.
8. **TASK-EK-006 / #327** (curiosity reward) — code/FSV-shipped; now available as a TASK-EK-018 priority-queue input. FSV `/mnt/d/contextgraph/fsv/task-ek-006-curiosity-fsv/curiosity_fsv.json` sha256 `cef7b6e576875fd1df8d92f849c4e4adf032ac6d834ac967e94b6eaddb30646f`.

### §6.3 — Tier 3 (Phase I apex: TASK-EK-018 atomic sub-issues)

Filed under #341. Execute roughly in this order:

10. **TASK-EK-018A** (dynamic-registration scaffolding) — must ship first; everything else in 018 depends on it.
11. **TASK-EK-018B** (absence-detection priority queue) — composes Tier 2 substrate; cheap once Tier 2 is done.
12. **TASK-EK-018C** (falsification gate) — reuses TASK-G-016 per-cell-promotion gate; ~1–2 weeks.
13. **TASK-EK-018F** (TransE-on-corpus-entity-graph first concrete instance) — the E11 replacement that proves the loop works end-to-end.
14. **TASK-EK-018D** (algorithmic-embedder synthesis) — reuses AlphaEvolve / TASK-EK-008 scaffold.
15. **TASK-EK-018E** (learned-head synthesis on E7).
16. **TASK-EK-018G, H, I, J** (audit, operator gates, VRAM, adversarial defense) — parallel.

### §6.4 — Tier 4 (Phase I non-018 EK tasks)

17. **TASK-EK-004 / #325** (mechanism-conjecture surface)
18. **TASK-EK-005 / #326** (counterfactual under invariant)
19. **TASK-EK-007 / #328** (held-out failure-mode class falsification) — extends to TASK-EK-018J
20. **TASK-EK-008 / #329** (AlphaEvolve bridge) — pattern reused by TASK-EK-018D
21. **TASK-EK-009 / #330** (ontology-growth audit) — companion to TASK-EK-018 promotion audit
22. **TASK-EK-013 / #335** (reverse-prediction)
23. **TASK-EK-014 / #336** (operator-contribution surface)
25. **TASK-EK-015** (credibility-weighted operator override; file when EK-014 green)
26. **TASK-EK-017 / #340** (library-level foundationality) — per-library deployment of TASK-EK-018
27. **016-ext** (embedder-foundationality)

### §6.5 — Tier 5 (Phase J generalisation; post-018)

28. J.1 — Non-Python language activation
29. J.2 — Non-Python fixture corpus activation
30. J.5, J.6, J.7 — ME-JEPA-X domain instantiations using TASK-EK-018 for embedder self-design per-domain

### §6.6 — Tier 6 (TASK-EK-018 v2)

31. 018-v2-A — Neural-architecture-search sub-case
32. 018-v2-B — Cross-library candidate proposals
33. 018-v2-C — Self-rewriting absence-detection priors
34. 018-v2-D — Status-reallocation extension for embedder-review

---

## §7 — Acceptance gates (the only ones that matter operationally)

### §7.1 — Phase G ship gate (the current gate)

**Gate:** `prediction_oracle_correlation ≥ 0.95` stable over **four consecutive rolling windows** on the 300×8 SWE-bench Lite Python mutation corpus, stratified by the 8 Python mutation categories. Per-cell exemptions allowed via the existing exemption policy.

**Current state:** `0.866667 < 0.95`. **Approximately 9.7% gap to close.**

**Closing levers (in expected ROI order):**
1. §1.3 calibration tasks completion.
2. TASK-EK-008 / #329 AlphaEvolve-pattern search over code candidates using source-backed `RealityPrediction` fitness.
3. §1.7 adversarial defense to ensure the claimed gate is defensible against adversarial probes.
4. Continued corpus-cadence Phase E self-optimization (already running).

### §7.2 — Phase I apex gate (the explanatory-knowledge gate)

**Gate:** TASK-EK-018 / #341 ships with the first concrete instance (TransE-on-corpus-entity-graph) auto-promoted into the panel as `EDynamic(1, "corpus_transe_v1")` with full provenance, operator approval, per-cell-improvement-verified, and the pairwise-MI matrix cardinality grown from 78 to 91.

**Current state:** Not started; substrate substantially in place.

**Closing levers:**
1. Ship TASK-EK-018A scaffolding.
2. Ship TASK-EK-018C falsification gate.
3. Ship TASK-EK-018F first concrete instance.
4. Operator approves the first auto-promotion.

### §7.3 — Phase J generalisation gate (post-018, ~1+ year horizon)

**Gate:** ME-JEPA-Voice OR ME-JEPA-Image template instantiation reaches a domain-appropriate 0.95-equivalent ship gate via TASK-EK-018-driven embedder self-design (no manual embedder curation post-bootstrap).

**Current state:** Not started; depends on Phase I apex.

---

## §8 — The honest commitments this roadmap makes

1. **The 0.95 ship gate is the only currently-falsifying claim.** Everything else is enabling or downstream. If the gate fails to close after Tier 1 ships, the predictor-architecture choice (not the EK programme) is wrong.

2. **TASK-EK-018 is the apex of EPIC #321.** Post-018, the explanatory-knowledge programme is operationally closed at the substrate level. Pre-018, the framework cannot claim Deutschian universal-explainer status at the substrate level (only at the per-task level via prior EK additions).

3. **The Phase J / ME-JEPA-X generalisation is operator-deferred until Phase I apex ships.** The current execution scope is Python ME-JEPA-Code first per `00_INDEX.md §Current Execution Scope`. Phase J is the natural next axis but is not active.

4. **No new architecture is required.** The entire roadmap can be executed on the existing crate workspace + the existing 13 active embedders + the existing frozen-instrument panel + the existing TCT machinery. The novel additions (TASK-EK-018 dynamic-embedder promotion) are wiring on top, not architectural rework.

5. **Operator-in-the-loop discipline is preserved throughout.** The catastrophic-class promotion gate (Phase E) applies to TASK-EK-018 embedder promotions. The two-distinct-operator-approval discipline applies to first-promotion + ship-gate-crossing promotions. The fail-closed gates apply to every new MCP tool. CLAUDE.md §15 invariants hold across all phases.

---

## §9 — How to use this doc

**For an AI agent picking up work:**
1. Read §6 priority queue end to end.
2. Find the highest-priority issue with status `pending` and no blocking dependency.
3. `gh issue view <N>` to read the issue body.
4. Follow `docs/futurebuild/specs/FSV-PROTOCOL.md` for the work itself.
5. On RESOLVED, update this doc's status column for that row.

**For the operator:**
1. Use §6 priority queue to decide what to assign next.
2. Use §7 to decide when each acceptance gate has closed.
3. Use §3.1 + §3.2 to decide which EK sub-issues to expand into atomic work.
4. Update §8 if the honest commitments change.

**When this doc becomes stale:**
Either (a) edit it in-place when issue numbers change or new tasks are filed, or (b) when the structure changes substantially, version this doc to `31_COMPLETE_ROADMAP_v2.md` and add a "see also" pointer.

---

## §10 — Source-of-truth for every claim in this doc

- Issue states: `gh issue list --state open --limit 100` (run before believing the status column).
- Commit references: `git log --since="2026-05-17 00:00" --oneline` for today's shipping.
- File-state SHAs: in the SITREP at `tmp/SITREP_2026-05-17_221150.md`.
- Spec hierarchy: `docs/futurebuild/00_INDEX.md`.
- Phase G plan: `docs/futurebuild/27_PHASE_G_ROADMAP.md`.
- LeCun-alignment plan: `docs/futurebuild/28_LECUN_JEPA_ALIGNMENT.md`.
- Reward-signal completeness plan: `docs/futurebuild/29_REWARD_SIGNAL_COMPLETENESS.md`.
- TASK-EK-018 canonical doc: `docs/futurebuild/30_SELF_DESIGNED_EMBEDDERS.md`.

---

**TL;DR.** Phase G (close 0.95 gate via Tier 1) → Phase I (build Tier 2 substrate + apex TASK-EK-018 via Tier 3 + non-018 EK via Tier 4) → Phase J (post-018 multi-language + ME-JEPA-X via Tier 5 + 6). Every leaf task is enumerated above with its GitHub issue number, current status, and priority tier. No task left out.
