# NORTH STAR — Why this project exists

> **Every line of code in this repository, every issue in this ledger, every embedder weight on aiwonder ZFS, every FSV evidence file in `/zfs/archive/contextgraph/fsv/` — all of it exists to make one numerical predicate evaluate `TRUE`. When it does, AI coding agents can ship Python patches without human review and the patches will work. That is the goal. That is the only goal.**

---

## 1. The plain-language statement

**ME-JEPA-Code is the deterministic binary reality predictor that lets AI coding agents stop generating unverified claims and start producing patches a calibrated verifier can trust.**

When a Claude Code / Codex / Cursor agent writes a code change, the system answers:

> Will this change exist in working reality? `Pass` or `Fail`.

When the answer is `Fail`, it explains why (named failure mode) and shows the closest historical patch that failed the same way. When the answer is `Pass`, it confirms the claim landed in bytes, the oracle agrees, the closest exemplars all passed, and two independently-encoded panels reached the same verdict.

When the system reaches the numerical predicate in [`TARGET-METRICS.md`](TARGET-METRICS.md) on the 300×8 SWE-bench Lite Python corpus for 4 consecutive rolling windows, **human review of AI-generated Python patches becomes statistically unjustified**. Production deployment begins. Auto-trust gates engage (#277). The post-0.95 creativity engine starts (#516 / #517 / #518 / #519 / #520).

That is the moment the system has worked. Nothing before that moment counts as "done." Nothing after that moment is the same project.

---

## 2. Why this matters (the operator-facing framing)

AI coding agents currently produce code at a rate humans cannot review at the same rate. The bottleneck is verification, not generation. A single Claude Code session can produce hundreds of edits per hour; a human reviewer reads at single-edit-per-minute pace on a good day. The asymmetry compounds: the more capable the agent, the wider the verification gap.

This is solvable if and only if a **deterministic verifier exists that correlates ≥ 95% with the Docker oracle on a representative corpus**, with calibrated per-cell confidence, Goodhart defenses, and online mistake-driven adaptation. Such a verifier replaces human review at the statistical level: the verifier's `Pass` carries the same information content as a human's `Pass`, the verifier's `Fail` carries the same information content as a human's `Fail`, and disagreements between the verifier and reality (when they happen) are captured as mistakes and feed back into the verifier's weights without human supervision.

This is what ME-JEPA-Code is. It is not a code generator. It is not a code reviewer in the LLM sense. It is a **mathematical verifier of code-against-reality** that runs in milliseconds on aiwonder's RTX 5090 and outputs `Pass | Fail` with calibrated confidence on every candidate patch an agent produces.

---

## 3. The framing: this is super-intelligence-shaped

Super-intelligence is not a model that writes better code. Super-intelligence is a system that:

1. **Predicts reality outcomes from observable inputs.** When the inputs and outputs are well-defined, this is a tractable engineering target.
2. **Closes the loop on its own mistakes.** Disagreement with reality materializes as a training signal without human supervision.
3. **Hits a hard numerical floor that defines functional super-intelligence in its domain.** When the floor is hit, the system performs beyond average human expertise at the task it was designed for.

ME-JEPA-Code's task is binary-Pass/Fail prediction on Python code changes against the Docker oracle. The hard numerical floor is the predicate in [`TARGET-METRICS.md`](TARGET-METRICS.md). Once that floor is hit, the system functions at super-human level in this exact domain — not because it understands Python more than any individual human, but because it predicts the Docker oracle outcome more accurately than any human reviewer empirically does on the same 300-instance corpus.

This is super-intelligence in the **operational** sense: the system replaces a human function (code review) at higher accuracy and at machine-scale throughput. The fact that it does so by predicting one specific number (`prediction_oracle_correlation`) does not make it less of a super-intelligence; it makes it a **falsifiable** super-intelligence, which is the only kind that can be engineered toward.

---

## 4. Why these specific numbers (not 0.90, not 0.99)

The thresholds in [`TARGET-METRICS.md`](TARGET-METRICS.md) are chosen for empirical sufficiency:

- **`0.95` Pearson** means the predictor disagrees with the Docker oracle on at most ~5% of holdout instances. At this threshold, the expected cost of false-positive Pass (predictor says Pass, oracle says Fail) is bounded by the cost of human-review-Pass under realistic operator throughput assumptions. Below `0.95`, false-positive rate climbs faster than the human-review-of-uncertain-cases bandwidth can absorb. Above `0.97`, marginal cost climbs faster than the marginal benefit because Docker oracle self-consistency may itself cap around `0.97-0.98` (BLINDSPOT-001 / #723 measures this).
- **4 consecutive windows** is the smallest interval at which the time-series statistic `corr ≥ 0.95` cannot be a flake under realistic distribution shift between calibration and serving. Fewer windows → drift-vulnerable. More windows → diminishing returns.
- **Panel B agreement within `0.05`** means the predictor cannot have memorized Panel A. If two independently-encoded panels (CodeBERT instead of tree-sitter, joern instead of attention, etc.) reach the same Pass/Fail verdict within 5%, the predictor learned about code, not about the panel.
- **`0.05` Brier / `0.05` ECE** are the standard calibration cutoffs below which probabilistic outputs are "well-calibrated" in the strict-proper-scoring-rule sense. Calibration matters because the predictor's `Pass with 0.93 confidence` must mean 93% of those cases are actually Pass in reality.

None of these are operator preference. They derive from the cost asymmetry between false-positive Pass (production regression) and false-negative Pass (wasted review of working patches), bounded by the Docker oracle's intrinsic noise floor.

#737 (BLINDSPOT-013) is the issue that formally justifies these specific thresholds. Until it ships, treat them as operator-locked working numbers with empirical heuristic support.

---

## 5. What this is NOT

- **Not a generative model.** No LLM inside (`feature = "llm"` triggers `compile_error!`). The system predicts whether an externally-generated patch works; it does not generate the patch.
- **Not a code reviewer.** It does not assess style, maintainability, idiomaticity, taste, or "is this a good change." Those are Q4 questions and explicitly out-of-scope (Q4 doctrine freeze #408, closed).
- **Not a benchmark harness.** The 300×8 SWE-bench Lite corpus is the training/eval substrate, not the product. The product is the predictor that runs in milliseconds on any candidate patch.
- **Not a research project.** Every architectural decision is gated by the numerical predicate. If it does not move at least one of the Tier-1/2/3/5/6 numbers in [`TARGET-METRICS.md`](TARGET-METRICS.md), or preserve a Tier-4 invariant, it must justify its existence on those grounds or be rejected.
- **Not multi-language at ship-gate time.** The first ship gate is Python-only on SWE-bench Lite 300×8. Other languages (#149 SWE-bench Verified, non-Python cells) are post-0.95 work.

---

## 6. How every other artifact serves the north star

| Artifact | Role in service of the north star |
|---|---|
| **#712 critical path** | Operator-authorized sequence of moves toward Tier-1 target #1 (`prediction_oracle_correlation ≥ 0.95`) |
| **#739 BLINDSPOT-AUDIT** | Epistemic audits checking whether the critical path is actually on track or assuming false premises |
| **#383 evidence-weighted ordering** | The locked rule that gate-active issue priority must be evidence about progress toward the predicate, not vision about what would be cool |
| **#397 consequence-prediction truth-system contract** | The product-contract definition: what shape the predictor's output must take |
| **#227 Python data explosion runtime policy** | Constraints on data pipeline that must hold while moving toward the predicate |
| **#633 manual full-system FSV gate** | Hardening pass ensuring evidence of progress toward the predicate is trustworthy |
| **#655 retire Rust FSV examples → manual SoT** | Verification protocol that prevents silent-success false readings of progress |
| **#683 trainer/checkpoint lineage (closed 2026-05-23)** | The hard prerequisite for any predictor weight ever moving toward target #1 |
| **#709 / #715 v2 9-slot static AST cache** | Substrate prerequisite for the predictor to learn from non-degenerate inputs |
| **#615 delete_test_call regeneration** | Corpus prerequisite for the weak `delete_test_call` cell to satisfy Tier-2 |
| **#618 / #619 doctrine repairs** | Tier-4 invariant restoration (anti-compensation, slot identity) |
| **#695 honest metric** | Tier-1 measurement integrity — replaces Goodhart rank-lift with raw Pearson |
| **#744 / #745 / #746 causal-embedder track** | Architectural addition conditioned on whether the panel-sufficiency probe (#734) shows the gap is causal-axis |
| **All 17 BLINDSPOT issues (#723-#739)** | Falsification audits — each could invalidate part of the strategy if its measurement fails |
| **FSV protocol (`FSV-PROTOCOL.md`)** | Verification contract — every claim that a number moved must be readable from the source of truth |
| **Manual SoT contract (`06 §1.9`)** | Anti-silent-success defense — operator-driven readback is binding for closure |
| **Aiwonder ZFS migration (#372, closed)** | Production substrate that durable evidence of progress lives on |
| **CLAUDE.md doctrine** | Operational invariants (Tier-4 of the target sheet) that must hold while moving toward Tiers 1-3/5/6 |

If any artifact in the project cannot be traced back to one of: (a) moving a Tier-1/2/3/5/6 number, or (b) preserving a Tier-4 invariant — it should not exist. Per #383, this is the evidence-weighted ordering rule.

---

## 7. The decision rule for every new piece of work — THE WORTHLESSNESS TEST

**If a proposed action does not move at least one of the Tier-1/2/3/5/6 numbers OR preserve a Tier-4 invariant, it is worthless and must not ship.**

This is not aspirational discipline. This is the hard contract that defines whether work belongs in this repository. Per **#383** evidence-weighted ordering (locked rule), any work that fails this test is rejected.

Before filing any new issue, writing any new code, training any new embedder, or proposing any new architectural change, every agent and every operator must answer:

1. **Which Tier-1/2/3/5/6 number does this move?** Be specific. "Improves the predictor" is not an answer. "Increases per-cell Pearson on `subtle_flip::python` from `0.80` to `0.85`" is an answer.
2. **By how much?** Estimate the expected delta. If the expected delta is < 0.005 on any target and the work cost is > 1 day, defer.
3. **What's the falsification condition?** Under what measured outcome would this work be classified as failed and rolled back?
4. **What's the doctrine cost?** Does this preserve every Tier-4 invariant? If not, it does not ship.
5. **Aiwonder execution path?** Per `CLAUDE.md §6.7`, every production action runs on aiwonder. Specify the `/zfs/archive/contextgraph/...` path, the SHA-pinning, and the manual SoT readback protocol per `06 §1.9`.

If the work cannot answer (1) cleanly with a specific number it moves, **it is worthless by definition** and should be reconsidered or rejected.

This applies to:
- New issues filed — if they don't trace to a number, close as `out-of-scope`.
- New code written — if it doesn't move a number or preserve an invariant, do not merge.
- New embedders trained — if expected δ_utility < 0.005, do not train.
- New corpora ingested — if they don't enable a number to move, do not ingest.
- New docs written — if they don't help future agents understand how to move a number, do not write them.
- New abstractions introduced — if they don't make a number-moving action easier, do not introduce them.

**The worthlessness test is the project's primary filter.** Activity is not the goal. Movement of these specific numbers is the goal.

---

## 8. The end-state behavior

When [`TARGET-METRICS.md`](TARGET-METRICS.md) evaluates `TRUE` for 4 consecutive windows on aiwonder:

1. `mejepa_ship_gate_status` reports `green` per-cell across all `(mutation_category × language)` cells.
2. `mejepa_promote_approval` engages operator first-promotion review (`CLAUDE.md §6.4 step 3`).
3. After operator approval, `CF_MEJEPA_ACTIVE_POINTERS` flips and the trained checkpoint becomes the serving predictor.
4. `mejepa_predict_latest` against any agent session begins returning `Verdict::Pass | Fail` with calibrated confidence, named failure modes on Fail, and Panel B agreement evidence.
5. Hooks (`scripts/cc/posttooluse-mejepa-capture.sh`) capture every test-outcome event into `CF_MEJEPA_PREDICTION_VERIFICATIONS` so any disagreement between predictor and oracle still materializes as a mistake-log row.
6. The mistake loop (#406 / #523) consumes those rows; the predictor's online head corrects same-panel future predictions; convergence target `mistake_repeat_rate ≤ 0.05` is monitored against the rolling 100-mistake window.
7. Per-cell drift guardrails (#173 post-green) engage. Any cell whose `prediction_oracle_correlation` drops below `0.93` triggers heal-scheduler retraining; below `0.90` triggers automatic rollback.
8. The post-0.95 creativity engine begins (#516 novelty oracle / #517 mistake-driven embedder demand / #518 retirement audit / #519 domain-routed sparse panel / #520 episodic-to-semantic consolidation) — autonomous bounded growth into the corpus's intrinsic dimensionality.

That is the end state. Everything before that moment is movement toward it.

---

## 9. The invariant that supersedes all targets

If at any moment a Tier-4 doctrinal invariant is breached — even momentarily, even in a test, even in a dev branch — the system has failed regardless of what the Tier-1/2/3/5/6 numbers say. Tier-4 invariants are not goals; they are **fences**. The system can be:

- below `0.95` on Tier-1 and still in service of the north star (it's progressing)
- at `0.95+` on Tier-1 but with `flat_vector_concat_used = true` and **immediately not in service of the north star** (it has broken doctrine)

This asymmetry is intentional. Numbers can be Goodhart'd. Invariants cannot — they either equal their required value or they don't. Tier 4 is the floor under the entire architecture. Without it, every other number is meaningless.

---

## 9.5 The state is not the constraint — only Tier-4 invariants are

**The current state of the project is the starting point, not the goal and not the constraint. If reaching the numerical targets requires expanding the test corpus, adding instrument slots, scaling the substrate, refining the oracle harness, or any other change to current state — that is the acceptable path.** The targets define the destination. The state is what we change to get there.

### What CAN change to reach the targets (state-change levers)

| Lever | Examples | Tier(s) it addresses |
|---|---|---|
| **Corpus expansion** | 300×8 SWE-bench Lite → SWE-bench Verified (~500) → SWE-bench Full (2,294) → CommitPack (4TB) → BugsInPy + ATOMIC + CauseNet | Tier-1 statistical power; Tier-2 per-cell support |
| **Panel expansion** | Add instrument slots; in-slot causal embedder replacement (#745); new domain-specific slots (#519) | Tier-5 panel sufficiency; Tier-1 ceiling |
| **Architectural refinement** | Iterative predictor head (#725); PanelGraph multi-chunk (#726); causal embedders (#744/#745/#746); pathway exploration (#416) | Tier-1 ceiling; Tier-2 calibration |
| **Calibration refinement** | Per-slot OOD (#389); conformal recalibration; Q5 calibration (#128) | Tier-2; Tier-3 |
| **Oracle refinement** | Replica-mode voting (#723); per-instance flakiness modeling (BLINDSPOT-014); per-test isolation (BLINDSPOT-007) | Tier-1 ceiling against oracle self-consistency floor |
| **Threshold revision** | 0.95 → 0.97 if oracle self-consistency proves higher than expected; or admit irreducible-noise carve-out for flaky cells | Tier-1 if BLINDSPOT-001 measurement justifies |
| **Substrate refinement** | Incremental forward cache (#733); v2 9-slot static cache (#715); aiwonder migration (#372 closed) | Tier-5 substrate quality |

### What CANNOT change (Tier-4 doctrine invariants — fences, not levers)

- Slot identity preservation
- Anti-compensation invariant (#394)
- Binary contract (Q1/Q2/Q5 only; Q4 frozen)
- No inner LLM (`feature = "llm"` compile guard)
- Aiwonder durable root (`/zfs/archive/contextgraph/...`)
- Doctrine §1.6 (formerly §1.5ter) — slot-preserving architecture
- Leakage policy (`target_labels_used_as_live_inputs = false`)
- ΔΞ.target_collapse = exactly 0

These are not levers because changing them would invalidate the entire architectural claim. Reaching the targets via Tier-4 violation = the system has not worked; it has cheated.

### Decision rule for changing system state

Before invoking any state-change lever, every agent and every operator must answer:

1. **Which target is currently stalled?** Specify the Tier + variable + current measurement + gap.
2. **Why does the current state make that target unreachable?** FSV evidence (e.g., #734 panel MI < 0.95 bits, #723 oracle floor < 0.95, statistical power CI > 0.05).
3. **What is the minimum state change needed?** Do not over-shoot. If 4× corpus expansion suffices for statistical power, do not propose 50× expansion.
4. **What Tier-4 invariants are preserved?** Enumerate. Any violation = the proposal does not ship.
5. **What is the aiwonder execution cost?** Wall-clock GPU days, durable-storage cost, operator review burden.
6. **What is the falsification condition?** Under what measured outcome does the state change get rolled back?

If a state change satisfies (1)-(6), file it as a `type:decision` issue with operator authorization (`CLAUDE.md §3` doctrine §3.10), execute on aiwonder per `CLAUDE.md §6.7`, verify via manual SoT readback per `06 §1.9`, and re-measure the targeted variable.

### Concrete examples of acceptable state changes

| Scenario | State change | Cost (aiwonder) |
|---|---|---|
| Statistical power on Pearson CI too loose at 240 holdout | Add SWE-bench Verified (500 instances → ~400 holdout); CI bounds to ~±0.029 at r=0.95 | ~3-5 days re-bake + oracle execution |
| Tier-5 BLINDSPOT-010 measures `I(panel; oracle) < 0.95 bits` | Add causal embedder slots (#745); or add new domain slots; or expand instrument family | ~3 weeks per slot + #733 incremental cache prereq |
| Tier-5 BLINDSPOT-001 measures `oracle_self_consistency = 0.93` | Implement per-instance flakiness modeling (BLINDSPOT-014); revise Tier-1 target to `≥ min(0.95, oracle_floor − ε)`; exclude flaky instances from gate scoring | ~1-2 weeks |
| Per-cell `delete_test_call` stuck at Pearson 0.69 | Regenerate corpus subset (#615 in flight); ~1,131 deterministic candidates via Docker oracle batch | ~3-5 days |
| Aggregate ECE stuck above 0.05 on weak cells | Per-cell conformal recalibration; per-cell calibration corpus expansion | ~1 week |
| Panel B not promotable from current state | Train Panel B encoders; deploy non-overlapping encoder set | ~4-6 weeks |

### What this principle prevents

- **State paralysis.** "We can't reach 0.95 on this corpus, so let's lower the threshold to 0.85." Wrong — if the answer is corpus expansion, do that.
- **Architectural ossification.** "Adding slots requires #733 first, so we can't add slots, so we can't reach Tier-5." Wrong — #733 itself is a state change; build it.
- **False scarcity.** "There's only 300×8 instances in SWE-bench Lite." Wrong — SWE-bench Full has 2,294, BugsInPy has 493, CommitPackFT has 742k, public causal substrate is ~5 orders of magnitude larger than current.
- **Doctrine creep.** "We need to relax slot identity to reach 0.95." Wrong — Tier-4 invariants are fences; reaching targets via doctrine violation is not reaching them.

### The summary rule

> **Reaching the numerical predicate is the goal. The state of the system is what we change to reach it. The doctrine invariants are what we never change. Anything else is movable.**

This rule, like all of `NORTH-STAR.md`, is enforced per #383 (locked rule, evidence-weighted ordering).

---

## 10. Cross-references

| Topic | Doc |
|---|---|
| **The numerical predicate** (target sheet) | [`TARGET-METRICS.md`](TARGET-METRICS.md) |
| **Project doctrine + Tier-4 invariants** | [`../../../CLAUDE.md`](../../../CLAUDE.md) |
| **Universal protocol** | [`../../../docs2/AICodingAgentSuperPrompt.md`](../../../docs2/AICodingAgentSuperPrompt.md) |
| **Per-turn agent contract** | [`../../../AGENTS.md`](../../../AGENTS.md) |
| **FSV verification protocol** | [`FSV-PROTOCOL.md`](FSV-PROTOCOL.md) |
| **System overview** | [`../01_overview.md`](../01_overview.md) |
| **Critical-path roadmap** | [`../08_roadmap_and_open_work.md`](../08_roadmap_and_open_work.md) |
| **Current measurement history** | [`../07_results_and_benchmarks.md`](../07_results_and_benchmarks.md) |

**Pinned GitHub context** for the north star: see `[P0] CONTEXT — Numerical target sheet (the perfect-state predicate)` issue + `[P0] CONTEXT — Sherlock-validated critical path (#712)`.

---

**This is the north star. Everything else serves it. When [`TARGET-METRICS.md`](TARGET-METRICS.md) evaluates `TRUE`, the system has worked. Until then, every action either moves a number, preserves an invariant, or should be reconsidered.**

**Last updated:** 2026-05-24. Update this doc only when the north star itself changes (a doctrine-level event, not a routine measurement update). Routine updates live in `TARGET-METRICS.md`.
