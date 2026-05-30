# ME-JEPA-Code — Optimal Strategy to Hit the Ship-Gate Benchmark

**Author:** strategic review (Claude Code session, 2026-05-29).
**Goal under review:** the North-Star predicate — `prediction_oracle_correlation ≥ 0.95`, stable × 4 rolling windows, per `(mutation_category × language)` cell, + Panel B ≥ 0.95 + `|corr_A − corr_B| ≤ 0.05`.
**Current measured baseline:** raw Pearson **0.4843**, Brier **0.0924**, ECE **0.0240** (`swebench-300x8-ship-gate-fsv`). Honest gap **0.4657** *against an assumed-perfect oracle*.
**Status:** synthesis of (a) the live project/issue state, (b) the operator's own 30-wave exa investigation queue (#755), and (c) fresh external research. Not part of the canonical futurebuild-8; a proposal to sequence the existing queue. Every recommendation maps to an existing issue or the worthlessness test.

---

## 0. The one-paragraph thesis

**The 0.4657 "gap" is partly an illusion created by measuring against an oracle assumed to be 1.0, and partly an aggregate that hides a bimodal per-cell reality.** The single highest-leverage thing to do — before any more training, corpus, or trainer work — is to run the **two cheap falsifier probes (#723 oracle self-consistency, ~2h; #734 panel-sufficiency MI, ~3–5d)**. The external literature now predicts both will move the goalposts: the oracle floor is demonstrably below 1.0 (OpenAI retired SWE-bench Verified over ~59% flawed hard-problem tests), and the achievable correlation for *execution-free* patch-outcome prediction tops out around AUC 0.80 in published work. Until #723/#734 land, every downstream decision (corpus expansion, new slots, architecture) is a bet placed blind. **Run the probes first; they can convert "we have a 0.47 gap" into either "we already shipped against the right yardstick" or "the panel is provably short N slots" — both of which change everything.**

---

## 1. Reframe the gate honestly (the part the docs under-weight)

### 1.1 The oracle is not 1.0 — the external evidence is now overwhelming
The gate measures agreement with the **Docker oracle verdict**, and the docs treat that verdict as ground truth. The 2025–2026 literature says otherwise:

| Source | Finding | Implication for the gate |
|---|---|---|
| **OpenAI, "Why we no longer evaluate SWE-bench Verified" (2026-02)** | ≥**59.4%** of audited hard problems have **flawed tests that reject correct solutions**; env-dependent spurious failures; train-on-solution contamination | The oracle's *validity* (verdict = truth) is low on hard cells; its *run-to-run self-consistency* is degraded by env/flakiness |
| **Aleithan et al. 2025, "Are Solved Issues Really Solved"** | solution_leak, weak tests, flaky tests; they filter flakiness by running each test **20×** | Flakiness is material; single-run labels are noisy |
| **SWE-ABS (2603.00520, 2026)** | **19.78%** of "solved" patches are semantically incorrect; test weakness widespread (77% of instances admit a surviving weakened variant) | Pass≠correct; the oracle is a *necessary-not-sufficient* signal |
| **"On Randomness in Agentic Evals" (2602.07150)** | pass@k vs pass^k gaps **up to 24.9pp** from stochasticity alone; recommends multi-run estimation | A single Docker run is a noisy draw; the gate needs replica-aware labels |
| **SWE-bench issue #280** | patches pass `fail_to_pass`/`pass_to_pass` yet fail unexecuted tests | The label contract itself under-tests correctness |

**What this means precisely.** ME-JEPA predicts the *Docker verdict*, not *true correctness*. The relevant ceiling is therefore the Docker oracle's **self-consistency** (same patch → same verdict across replicas), which is degraded mainly by **flakiness** (env, ordering, randomness), not by the validity problem. The literature predicts that self-consistency sits in roughly **[0.93, 0.99]** with a non-trivial tail of flaky instances. **You cannot correlate with the oracle above its own self-consistency.** So the honest gate is `min(0.95, oracle_self_consistency − ε)` with flaky instances quarantined — exactly the re-frame #723 specifies and #754 authorises (threshold revision is allowed *toward the empirical floor*, with irreducible-noise carve-outs; only *arbitrary lowering* is forbidden).

→ **Action P0-A: run #723 now (~2h aiwonder Docker).** 30 instances × 10 replicas. This is the cheapest, highest-leverage measurement in the entire project and the literature says it will change the gate's framing.

### 1.2 The aggregate 0.4843 is bimodal — stop treating it as one number
The 8 mutation cells are not one population:

| Cell | Oracle variance | Difficulty | Right metric (#532) | Likely state |
|---|---|---|---|---|
| `known_good` | ~all-pass | easy | pass-recall + confidence | near-passing |
| `compile_error` | all-fail | trivial (parse) | fail-recall + Wilson LB | near-passing |
| `wrong_file` | all-fail | easy | fail-recall | near-passing |
| `over_engineer` | ~all-pass | easy | pass-recall | near-passing |
| `swap_variable` | mixed | medium | Pearson + Fisher-z | contested |
| `off_by_one` | mixed | **hard** | Pearson | weak |
| `subtle_flip` | mixed | **hard** | Pearson | weak |
| `delete_test_call` | **structurally pass-sparse** (confirmed **32 pass / 9,079 fail = 0.35%** in the 2026-05-29 oracle batch) | n/a | **degenerate fail-recall, NOT Pearson** | mislabeled as a Pearson cell |

**Two corrections fall straight out of this:**
1. **`delete_test_call` should not be a Pearson cell.** The just-completed 9,111-candidate oracle run proves it is ~99.6% fail — deleting an assertion almost never flips `FAIL_TO_PASS`. Forcing Pearson on a 0.35%-positive cell is a category error (Pearson is degenerate at near-zero variance). Per #532 it should use **fail-class recall + Wilson lower bound + Brier/ECE**, or take a documented **cell exemption** (`config/cell_exemptions.toml`). **Stop running more `delete_test_call` oracle batches (#808 lineage) — the 32/9079 result is conclusive; more candidates won't manufacture pass-support that the mutation operator structurally cannot produce.** This is the clearest "stop doing low-leverage work" finding in the review.
2. **The real battle is 2–3 mixed hard cells** (`subtle_flip`, `off_by_one`, maybe `swap_variable`). Aggregate movement comes almost entirely from these. Strategy must be **per-cell, not aggregate.**

---

## 2. The reachability decision tree (run before committing the plan)

This is the spine. Everything branches on #734 (panel MI) and #723 (oracle floor).

```
#734  I(panel; oracle_outcome)         #723  oracle_self_consistency
        │                                       │
   ┌────┴─────┐                          ┌──────┴───────┐
 ≥0.95 bits  0.7–0.95   <0.7          ≥0.97        [0.93,0.97)        <0.93
   │          │          │              │              │                │
 panel      add 1–2    ARCHITECTURAL  gate 0.95     re-frame gate     re-design
 sufficient  slots in   PIVOT —        valid as-is   to floor−ε +      oracle harness
 → execute   parallel   slots are                    quarantine flaky  (replica vote)
   #712      with #712  pre-0.95 P0                   instances         or carve-out
```

- **Most likely real-world outcome** given the literature: `#734` lands `0.7–0.95` (panel marginally short on the hard cells — concurrency/multi-file/runtime-env signal missing per BLINDSPOT-004/006), and `#723` lands `[0.93, 0.97)` (flaky tail). That combination means: **(a) add 1–2 informative slots, (b) re-frame the gate to the empirical floor with flaky quarantine, (c) then the downstream #712 work is worth doing.**
- The probes are **independent of the trainer/substrate** (they run against any frozen forward cache + the existing Docker harness), so they can run *today* in parallel with everything else. There is no reason to defer them.

---

## 3. The optimal sequence (evidence-weighted, maps to existing issues)

Ordered by **(leverage on a Tier-1/2 number) ÷ (cost)**, with the falsifiers first because they can void or redirect everything after.

### Phase 0 — Falsify or confirm reachability (days, do immediately, parallel)
| Rank | Action | Issue | Cost | Decides |
|---|---|---|---|---|
| 0.1 | **Oracle self-consistency** — 30×10 replica vote | **#723** | ~2h | the gate's true ceiling + flaky-instance quarantine list |
| 0.2 | **Panel-sufficiency MI** — MINE/InfoNCE/Kraskov `I(panel;oracle)`, global + per-cell + per-slot drop | **#734** | ~3–5d | whether 0.95 is reachable on current slots; which slots carry signal |
| 0.3 | **Clean substrate** so 0.1/0.2 run on valid inputs: inter-embedder distinctness gate + v2 9-slot static AST cache | **#709 + #715** | ~6–8h/slot | removes the `e8≡e1` aliasing that would corrupt the MI estimate |

> Why MI first, before corpus expansion: **expanding the corpus when the panel is information-insufficient is wasted compute.** Fano/Kovalevsky bounds (Meyen 2016; Frénay) make this formal — for binary `H(Y)≈1 bit`, achievable accuracy is capped by `I(panel;oracle)`. #734 already specifies MINE (Belghazi 2018) / InfoNCE (van den Oord 2018) / Kraskov; the classifier-based MI estimator (Donsker-Varadhan, arXiv:1906.01824) gives a conservative *lower bound* — ideal for a "is it provably insufficient" test.

### Phase 1 — Honest measurement + calibration (1–2 weeks; do regardless of Phase 0 outcome)
| Action | Issue | Why |
|---|---|---|
| **Replace rank-lift with raw Pearson + per-cell calibration.** Kill the 0.79 Goodhart transform from the gate metric. | **#695** | The 0.79 number is an exemplar-similarity magnifier that doesn't improve Brier/ECE; it hides the real gap. Honest metric is a precondition for trusting all later movement. |
| **Per-cell calibration: isotonic (mixed cells) + Venn-Abers / reliably-calibrated isotonic (imbalanced cells) + Mondrian conformal per cell.** | INV-016/017/018/020, #695 | External: isotonic beats Platt for non-sigmoid miscalibration but needs samples; for pass-sparse cells use Venn-Abers or reliably-calibrated isotonic with credible intervals (Springer 2021; arXiv:2410.18144). Mondrian conformal gives per-cell coverage. |
| **Brier-/ECE-aware training loss** (soft-ECE or Brier-weighted gradient). | INV-014/015 | SWE-RM (arXiv:2512.21919) shows calibration (ECE) + discrimination (AUC) are the metrics that actually transfer, not top-1 ranking. ME-JEPA already tracks Brier/ECE — train on them directly. |

### Phase 2 — Close the hard mixed cells (the real gap; weeks)
This is where the 0.95 is actually won or lost. Two complementary levers:

**2a. Statistical power via corpus expansion** (only after #734 says the panel *can* support 0.95):
- Add **SWE-bench Verified (~500)** → Pearson 95% CI tightens from ±0.062 to **±0.036** (#754 math). ~3–5 days. This is the cheapest power lever and the right first expansion.
- Defer SWE-bench Full (2,294 → ±0.014) until Verified shows the per-cell metric is moving in the right direction. Don't pay 10–14 days blind.
- Add **BugsInPy (493 real Python bugs)** for cross-distribution validation of the hard cells (different oracle contract; needs adapter work) — this also de-risks the "controlled mutations are too easy" critique.
- Training-only scale (CommitPackFT 2GB, TSSB-3M) for the predictor head — *not* gate corpus.

**2b. Architecture levers that lift correlation on contested cells** (pick by #734 per-slot result):
| Lever | Issue / source | Expected effect | Note |
|---|---|---|---|
| **Group-relative / contrastive scoring** instead of pointwise | R4P (arXiv:2510.22775); INV-012 | R4P's group-wise objective beat pointwise + reduced reward-hacking, 72.2% on SWE-bench Verified > o3. Score each candidate *relative to its cell-peers* | High-leverage, doctrine-safe (no LLM), composes with slot-preserving panel |
| **Iterative-refinement predictor head** | #725 / INV-006 | Re-reads its own residual; helps the contested cells | |
| **Execution-aware features** (trace/coverage already a slot) | INV-010 (TRACED/DyPro) | The hard cells (subtle_flip/off_by_one) are runtime-semantic; static slots may be MI-insufficient → this is the slot to add if #734 says "marginal" | Directly addresses the likely #734 verdict |
| **GNN code-property-graph slot** | INV-009 | Adds structural signal orthogonal to existing slots | Gate via #516 novelty (max-corr<0.7) + #518 utility (lift≥0.005) |
| **New slots only if #734 < 0.95**: concurrency-trace, dep-version, runtime-env | #734 → #519/#745 | These are the BLINDSPOT-004/006 "missing slots" most likely responsible for a marginal MI verdict | In-slot replacement (#745) avoids schema change |

### Phase 3 — Stability + mistake loop + Panel B (the remaining 3 of 4 Tier-1 numbers)
| Action | Issue | Why |
|---|---|---|
| **Repair mistake-loop countability** (LUT inertness) then DER++/A-GEM/Synaptic-Intelligence as Fisher alternatives | #689, INV-023/025/029/030 | Tier-3 convergence is currently unmeasurable; external CL methods (DER++, A-GEM) are drop-in and don't require a durable Fisher row (which #524 guards) |
| **4-window burn-in** once per-window variance < 0.03 | #167 | Tier-1 #2; needs Phase-2 power first (else windows flake) |
| **Panel B encoder training** (CodeBERT/Voyage/bge disjoint set) | #405/#521, INV-008 | Tier-1 #3 + #4; the anti-Goodhart guard. ~4–6 weeks; start once Panel A clears 0.95 on the hard cells so you're not triangulating a moving target |

### Phase 4 — Eliminate the honesty/Goodhart hazards (parallel, cheap, gate-protecting)
Clear the no-op/hardcoded-telemetry issues so future gate readings are trustworthy: **#685** (zeros-regardless logits), **#687** (no-op detach_loras), **#688** (hardcoded best/worst), **#690** (hardcoded retrain_triggered), **#621** (hardcoded gtau_pass_rate 1.0), **#699** (pseudo-cert train health), **#618/#619** (flat-panel projections — doctrine §1.6). None move the metric directly, but each is a place a future 0.95 could be faked. Do them alongside Phase 1.

---

## 4. What to STOP doing (negative recommendations — equally important)

1. **Stop running `delete_test_call` oracle batches** (#615/#808/#796 lineage). The 32/9,079 result is conclusive: the operator confirmed this morning's run added zero usable pass-support. Convert the cell to the #532 degenerate contract or exempt it. *(This frees the aiwonder GPU, which is currently idle anyway.)*
2. **Stop quoting / gating on the 0.79 rank-lift.** It's a Goodhart magnifier (#695). All planning uses raw Pearson 0.4843.
3. **Don't expand the corpus before #734.** If the panel is information-insufficient, more rows are wasted GPU-days.
4. **Don't lower 0.95 arbitrarily.** The *only* legitimate threshold change is re-framing to the empirical oracle floor (`min(0.95, floor−ε)`) with flaky-instance quarantine, justified by #723 data. That's not lowering the bar; it's measuring against the right one.
5. **Don't add an LLM verifier** despite SWE-RM/CodeScaler/R4P being LLM-based — the no-inner-LLM invariant is a Tier-4 fence. Borrow their *ideas* (group-relative objective, ECE-first evaluation), not their architecture.

---

## 5. The realistic end-state (calibrated expectations)

- **If #734 ≥ 0.95 and #723 ≥ 0.97:** the current architecture can hit the gate; execute Phases 1–3; estimated **~6–10 weeks** of aiwonder work, dominated by Panel B (4–6 wks). Probability: **moderate** — the literature's AUC-0.80 ceiling for execution-free static prediction is a headwind, but ME-JEPA's controlled-mutation corpus is easier than real APR patches, and execution-aware slots can lift the hard cells.
- **Most likely (#734 ∈ [0.7,0.95), #723 ∈ [0.93,0.97)):** ship gate is reachable **after** adding 1–2 execution/runtime slots **and** re-framing to the empirical floor with flaky quarantine. ~10–14 weeks. This is the honest base case and it is still a *win* — a calibrated verifier at the oracle's own ceiling is exactly the North-Star deliverable.
- **If #734 < 0.7:** the 15-slot static panel is provably underpowered for the hard cells; the post-0.95 creativity engine (#517/#519 sparse domain-routed panel + mistake-driven slot demand) must move pre-0.95. This is a pivot, not a failure — and it's far better to learn it from a 3–5 day MI probe than from 3 months of training that can't converge.

**In all three branches, the first move is identical: run #723 and #734.** That is the optimal strategy's invariant core.

---

## 6. The 5 concrete next actions (in order)

1. **#723** — oracle self-consistency, 30×10 replicas, ~2h aiwonder. *Re-frames the gate.*
2. **#709 + #715** — distinctness gate + v2 9-slot cache, so the MI probe runs on clean inputs.
3. **#734** — `I(panel; oracle)` global + per-cell + per-slot-drop. *Decides the whole strategy branch.*
4. **#695** — kill rank-lift, install raw Pearson + per-cell isotonic/Venn-Abers calibration. *Honest yardstick.*
5. **Reclassify `delete_test_call`** to the #532 degenerate contract / cell exemption, and **stop its oracle batches.** *Frees effort + removes a structurally-unwinnable Pearson cell.*

Then branch per §2's decision tree.

---

## 7. Mapping to existing issues (nothing here is new scope)

Every recommendation is an existing issue or the operator's own INV queue: falsifiers **#723/#734**; substrate **#709/#715**; metric **#695**; calibration **INV-014..022**; architecture **#725/INV-006/009/010/012**; corpus **#754/INV-001..004**; mistake-loop **#689/INV-023..031**; Panel B **#405/#521/INV-008**; honesty **#685/#687/#688/#690/#621/#699/#618/#619**; degenerate-cell contract **#532**; threshold re-frame **#723/#754**. The contribution of this doc is **sequence and stop-list**, weighted by fresh external evidence (oracle-floor reality, AUC-0.80 execution-free ceiling, group-relative > pointwise, calibration > ranking, Fano/MI reachability). The single most important change vs. the current #712 ordering: **promote the two falsifier probes (#723/#734) to rank 0 — ahead of the substrate/trainer chain — because the external literature now predicts they will change what "0.95" even means.**

---

### Sources (external, this review)
- SWE-RM, *Execution-Free Feedback for SWE Agents* (arXiv:2512.21919) — calibration+AUC > ranking; execution-free verifier SOTA.
- CodeScaler (arXiv:2602.17684); R4P / *Patch Reasoner* (arXiv:2510.22775) — group-relative verifier, 72.2% on SWE-bench Verified.
- APOSTLE (ESWA 2024) / Panther-Leopard (arXiv:2203.08912) — static patch-correctness prediction ceiling ~AUC 0.80.
- OpenAI, *Why we no longer evaluate SWE-bench Verified* (2026-02) — ≥59% flawed hard-problem tests.
- Aleithan et al. 2025 (arXiv:2503.15223); SWE-ABS (arXiv:2603.00520); *On Randomness in Agentic Evals* (arXiv:2602.07150); SWE-bench issue #280 — oracle noise / flakiness.
- Meyen 2016 (MI↔accuracy bounds); Frénay (MI feature-selection inadequacy); classifier-MI (arXiv:1906.01824); MINE (Belghazi 2018); InfoNCE (van den Oord 2018) — #734 estimator toolkit + Fano/Kovalevsky reachability.
- Calibration: Springer survey (s10994-023-06336-7); reliably-calibrated isotonic (Springer 2021); Platt-after-undersampling (arXiv:2410.18144); imbalanced-calibration measure (researchsquare rs-3242706).
