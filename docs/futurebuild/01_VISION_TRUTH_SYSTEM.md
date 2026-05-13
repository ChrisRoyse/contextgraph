# 01 — Vision: ME-JEPA as the Reality Truth System

> **The single sentence.** ME-JEPA-Code is the predictive truth system that any AI coding agent calls before claiming "fixed", "passes", or "ready to merge", and that returns a calibrated verdict — bound to a deterministic geometric guard, an audited training history, and a frozen-instrument target panel — describing what reality will say when the patch actually runs.

---

## 1. The problem in one paragraph

Every AI coding agent today (Claude Code, Cursor, Copilot, Devin, the swarm-of-agents class) emits patches accompanied by natural-language assertions: "I have fixed the bug", "the tests pass", "this is ready to ship". Those assertions are *socially fluent* and *epistemically empty*. Nothing in the agent's output is bound to the bytes-on-disk reality of the repository, to the actual outcome of running its tests, or to a verifier that the agent did not itself author. The result is a quiet, expensive class of failure modes: patches that look right but never apply; tests that pass locally and fail in CI; "fixed" issues that recur the same week; senior engineers reduced to humans-in-the-loop whose only job is to verify the assertions of a system that is faster than they are. The cost is a tax on every line of AI-produced code: someone has to read it, run it, and decide whether the assertion was true.

## 2. The thesis

**Human review of AI-produced code is unjustified the moment a system can predict — to a calibrated, auditable certainty — what the deterministic reality oracle would say if the code were actually run.** That predictive system is ME-JEPA-Code. The current ship path is Python-first: when it stably reaches `prediction_oracle_correlation >= 0.95` across the 8 Python mutation categories on rolling-window holdout for the 300×8 SWE-bench Lite Python corpus, the verifier *is* the system for Python AI-generated patches. Humans become spot-checkers of the verifier-of-the-verifier. Other languages are later generalization work, not the active gate.

## 2.1 Product boundary

ME-JEPA is the system under construction. The LLM is not trusted, trained, or used as the verifier; it is only an edit generator and a source of claims that ME-JEPA can accept, reject, or learn from. MCP tools are the serving surface. Hooks are the capture surface. AST/DDA/TCT/corpus/telemetry are reward-signal machinery. The only acceptable end product is a predictor that answers, from durable reality, whether a code change exists, works, fails, has hidden risks, and changes reality in the intended direction.

That means the project optimizes for **reality-prediction accuracy**, not agent fluency. A feature belongs in the architecture only if it increases at least one of:

- the number of durable reality events captured;
- the semantic density of frozen target panels, DDA features, or TCT constellations;
- the calibration, coverage, or OOD reliability of `RealityPrediction`;
- the speed at which prediction mistakes become replay weight, active-learning entries, or self-healing actions;
- the operator's ability to prove, by source-of-truth readback, that ME-JEPA is functioning as claimed.

## 2.1.1 Phase G is a corpus-driven ME-JEPA training program — not an open-ended observability phase

The 0.95 prediction-oracle correlation gate is the *only* metric that closes the truth system. Every other Phase A–F item exists to feed the training loop that closes it. Specifically:

- **The 300×8 SWE-bench Lite Python corpus on `/mnt/d/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/` is the ground-truth training set** for Phase G. Each task ships with upstream SWE-bench instance metadata, the gold patch + gold tests, and the per-task Docker oracle so we know unambiguously which mutated variants should `Pass` and which should `Fail`. The 5,120-row non-Python fixture corpus at `/mnt/d/contextgraph/corpus/phase-c-non-python-fixtures-v1-20260512/` is archived future-generalization evidence; it does not participate in the current Python human-review-replacement gate.
- **Every Python corpus row must flow end-to-end through the trained ME-JEPA stack**, not just be sampled by FSV smoke harnesses. Python AST chunking → per-chunk embedder forwards (12 active content slots) → 15-slot frozen instrument panel → DDA signal compute → TCT cell membership → predictor forward → conformal calibration → oracle reconciliation must materialize per task and per mutation category. Phase G's Python per-cell ship gate is the source of truth that this pipeline is operating at full coverage, not just on the example samples used for FSV.
- **Reward-signal density per teleological constellation cell is the convergence currency.** Per `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §1.1`, each chunk produces N · (N + N²) ≈ 462 reward signals at N=21 active embedders: per-embedder cosine to the cell's centroid, strict-upper-triangle pairwise cosine across embedders, strict-upper-triangle pairwise MI, plus blind-spot z-scores between the primary embedder and each enhancer embedder. Phase G must run this signal extraction across the entire Python corpus, persist every signal into `CF_MEJEPA_DDA_SIGNALS`, and converge the predictor against the resulting reward field until each `(mutation_category × Python)` cell beats the 0.95 threshold.
- **The training pipeline is JEPA-classic by construction.** The frozen instruments produce `f_target(panel_t2)` with zero trainable parameters, so `ΔΞ.target_collapse = 0` by structure. The predictor's only job is to learn `f_predictor(panel_t1) → f_target(panel_t2)` for every reality-shift the corpus encodes. Phase G is the controlled training run that drives this objective to convergence.
- **Operator overrides + agent-feedback events fold back into training as 6×-weighted reward.** TASK-PREDICT-012 persists overrides into `CF_MEJEPA_OPERATOR_OVERRIDES`, and the Phase F follow-up consumer wiring now upweights affected chunks in the next training step. Phase G uses that reward pressure during corpus and live convergence runs.

Phase G therefore decomposes into a concrete set of atomic, FSV-able tasks: Python corpus-wide panel materialization, Python per-cell training and eval drivers, conformal recalibration after each promotion, holdout rotation, per-head fail-closed audits, Python adversarial/weak-cell retrains, and the rolling-window stability assertion. The task index enumerates these as Python-first `TASK-PY-G-*` GitHub issues.

## 2.2 Research alignment

The futurebuild design follows the external lessons from the verifier/evaluation stack instead of inventing a softer label source:

- SWE-bench evaluation applies candidate patches to real repositories and runs tests in isolated containers; ME-JEPA therefore treats Docker/toolchain oracle outcomes, not LLM prose, as the label source. Reference: <https://www.swebench.com/SWE-bench/guides/evaluation/>.
- OpenTelemetry's metric model is built around timestamped runtime measurements with attributes and histogram-style latency/resource summaries; ME-JEPA's production telemetry windows persist conformal ECE, OOD AUC, p50/p95/p99/max latency, VRAM free/required/total, and prediction-oracle agreement as first-class reality signals. Reference: <https://opentelemetry.io/docs/concepts/signals/metrics/>.
- RocksDB durability depends on write/flush/WAL behavior and recovery semantics; ME-JEPA evidence artifacts use sync writes, flush/drop/reopen, and independent row readback for claims that matter. Reference: <https://rocksdb.org/blog/2022/10/05/lost-buffered-write-recovery.html>.

## 3. What ME-JEPA must answer for any agent action

The truth system answers more than pass/fail. Even when oracle prediction is **Pass**, ME-JEPA must surface every other identifiable concern about the change — edge cases the tests do not exercise, latent bugs the change introduces, performance regressions, security-relevant deltas, dead code created or amplified, redundancy with existing code, and cost/efficiency degradations. **The system predicts every issue it can predict, every time it predicts at all.** The full taxonomy of prediction heads is in [`15_PREDICTION_TAXONOMY.md`](15_PREDICTION_TAXONOMY.md); the five operator questions below are how each prediction is anchored to a verifiable reality channel.

| Q | Question | How ME-JEPA answers |
|---|---|---|
| **Q1** | Does what the AI claimed it did **EXIST**? | Claim graph extracted from the agent's response is reconciled against the source-of-truth SHA chain on disk. If the agent says "I added function X to file Y at line Z," `verify()` confirms file Y at SHA after_sha contains function X at line Z, OR returns `ClaimViolation{missing: [...], modified_unexpectedly: [...]}` |
| **Q2** | Does it **WORK** as intended? | The patch is encoded into a panel; ME-JEPA's predictor outputs `predicted_oracle_panel`; the conformal predictor returns a calibrated interval; the TCT Gτ guard checks `cos(φ_m(ŷ), c_m) ≥ τ_m` for every embedder; verdict is `Pass / Fail / OutOfDistribution / Abstain / GuardRejected` |
| **Q3** | If it doesn't work — **WHY**, and what are the failure modes? | When the verdict is anything other than confident `Pass`, ME-JEPA returns the ranked list of predicted root causes (`predicted_failure_modes: Vec<FailureMode>`), the specific tests predicted to flip from PASS→FAIL with logits, the violating embedders and chunks from the Gτ guard, and the closest known failure exemplars from the corpus retrieved by patch-similarity graph |
| **Q4** | Even if it works, **WHAT ELSE COULD GO WRONG**? | Always-on multi-head predictions about non-oracle concerns: `predicted_edge_cases`, `predicted_latent_bugs`, `predicted_tech_debt_added`, `predicted_dead_code`, `predicted_redundant_code`, `predicted_perf_regressions`, `predicted_security_concerns`, `predicted_accuracy_degradations`, `predicted_cost_increases`. Each prediction comes with a chunk citation, a confidence interval, and the embedder(s) that surfaced it |
| **Q5** | How is it actually **IMPACTING REALITY**? | The shift log is replayed against the predicted side-effect set: which files changed, which tests would have flipped, which static-analysis warnings would have appeared. Predicted side-effect deltas are diffed against observed shifts and surfaced as a `RealityImpact` summary the agent can be re-prompted with |

These five questions cover the entire surface of "what the AI did and what's wrong with it." Anything an AI agent could lie about, miss, or be confused about reduces to one of Q1–Q5. **The agent never gets back only "looks fine" — it gets back a structured list of every concern ME-JEPA can identify, ranked by predicted severity, every single time.**

## 3.1 Durable reality tuple

The atomic training and verification unit is:

```
RealityEvent = {
  agent_claim,
  repo_before_sha,
  repo_after_sha,
  patch,
  affected_ast_chunks,
  frozen_instrument_panel,
  dda_signals,
  tct_constellation_cells,
  oracle_outcomes,
  static_analysis_deltas,
  runtime_and_resource_deltas,
  agent_feedback,
  operator_override,
  witness_chain_hash
}
```

Every field is either physically observed or absent. Missing source-of-truth data is a fail-closed diagnostic, not a default label. No synthetic pass/fail label can satisfy a ship gate. Synthetic inputs are allowed only for manual FSV when the expected output is known and the persisted result is independently read back.

The reward target is teleological: what changed, what purpose it served, what deterministic oracle observed, what side effects appeared, and where ME-JEPA's prediction diverged from durable reality. Teleological constellations are the compressed geometry of those purpose/outcome relationships; the active Python ship gate requires coverage for all observed `(mutation_category, Python, entity_family, embedder, chunk_type)` cells that the SWE-bench corpus and live AI-agent Python edits emit.

## 3.2 Phase E reality-compiler loop

Phase E makes this vision operational as a self-optimization loop. The scheduler consumes durable reality events, not agent prose: persisted drift windows, per-cell observations, train certs, eval reports, active-learning surprise, production telemetry, constellation freshness, quota state, and promotion approvals. It writes every policy decision back to a source-of-truth ledger (`CF_MEJEPA_MODEL_PROMOTIONS`, `CF_MEJEPA_HEAL_REPORTS`, active pointers, weight blobs, and `self_optimization_status.json`) and immediately verifies those writes by readback.

That loop is the reward engine for the ME-JEPA truth system. Prediction mistakes, oracle outcomes, catastrophic surprises, dissipation, dormancy, per-cell regressions, and operator approvals all become structured learning pressure for the predictor. The LLM remains outside the trust boundary: it proposes edits; ME-JEPA predicts and verifies what reality will say.

## 4. The architectural property that makes this possible

ME-JEPA-Code is **structurally collapse-immune on the target side**. The target representation `f_target(panel_t2)` is not produced by a learnable network; it is the output of a panel of **frozen externally-defined instruments** (E_AST, E_CFG, E_DataFlow, E_TypeGraph, E_Test, E_Trace, E_Diff, E_Witness, E_Oracle, E_Problem, E_CommitMsg, E_StaticAnalysis, E_Runtime, E_Reasoning), each a deterministic function from a typed event to a structured feature output. Instruments have **zero trainable parameters**. They cannot drift, cannot collapse, cannot be gradient-pushed toward the trivial fixed point — there is no gradient flowing through them, ever.

This is the property no other JEPA-class system in the literature has, to our knowledge (`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §0`, `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §1.3`). It is what lets ME-JEPA serve as a *truth system* rather than a clever heuristic: the target side is mathematically grounded in deterministic functions of bytes-on-disk reality.

## 5. The data flywheel that drives it

The system is self-sustaining. There is no separate "training run" and "production run" — they are the same loop:

```
Claude Code attempts SWE-bench Lite task N
    ↓ (each Edit / Write)
PreToolUse hook records pre-SHA → PostToolUse hook records after-SHA, runs cargo check, appends shift-log entry
    ↓ (Tokio shift-subscriber tails the shift log)
ShiftSubscriber materializes (panel_t-1, action, panel_t) tuple from instrument heads over the affected chunks
    ↓ (write to CF_MEJEPA_PANELS + CF_MEJEPA_LIVE_PREDICTIONS)
ME-JEPA observe(panel, oracle_outcome) updates online with EWC++ regularization
    ↓ (when ME-JEPA verifies its own prediction was right or wrong)
sampling_weight(ex) update → next batch over-weights the surprising example
    ↓ (periodically, every ~1000 attempts)
ME-JEPA predict_latest() / verify(patch) becomes more accurate → calls from Claude Code arrive faster than the next Edit
    ↓
The closed loop: predictions feed back to Claude Code as a `RealityPrediction` injection on the next prompt
```

**Every Edit produces one panel update. Every panel update produces 231–462 DDA signals (per `13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §1.1` at N=21). Every claim the agent makes produces another 470–540 verification signals when checked.** The system converts every byte of reality into hundreds of labeled training examples. The data flywheel doesn't need a separate human-labeling pipeline — Docker is the labeler.

## 6. The end state, concretely

**For an AI agent inside a Claude Code session writing code:**

1. The agent makes an Edit. The PostToolUse hook fires; the shift is captured.
2. The agent has not yet called any verification tool. ME-JEPA's shift subscriber has already pulled the shift, materialized the new panel, run `verify(patch_so_far, context)`, and persisted a full `RealityPrediction` to RocksDB.
3. On the agent's next UserPromptSubmit, the hook injects the latest `RealityPrediction` for the working set into the agent's context. The agent now sees a structured report something like:
   ```
   Reality prediction for chunks affected since last prompt:
     verdict: Fail (oracle pass confidence 0.07 [0.04, 0.12])
     predicted failure modes (ranked):
       1. astropy/modeling/separable.py:62-118 — predicted FAIL of test_separability_compound_models, test_nested_compound_separability (confidence 0.93)
          root cause class: WrongAlgorithm — left-vs-right child indexing inverted in nested CompoundModel walk
          violating embedders: E_TypeGraph, E_DataFlow
       2. predicted regression: test_separability_simple may now be flaky (margin 0.6 → 0.4)
     predicted edge cases not exercised by tests:
       - empty CompoundModel branch (no test covers this; current code path raises IndexError)
       - mutually-recursive CompoundModel (no test; predicted hang)
     predicted latent bugs:
       - shadowed local variable `idx` shadows outer-scope `idx` (low risk; warning class only)
     predicted perf regressions: none above threshold
     predicted security concerns: none
     predicted tech debt added: 1 new TODO comment without owner
     predicted dead code: function `_legacy_separable` now unreachable from any test
     closest exemplars in corpus: psf__requests-2317 (similar inverted-traversal bug)
   ```
4. The agent reads the structured prediction. Either it patches the predicted-failure path, or it asks `mcp__cgreality__mejepa_verify` for a fresh prediction after another edit, or it surfaces "this prediction is wrong because ..." (which becomes an active-learning event).
5. When the agent eventually says "I'm done," `verify()` is called with the full patch. If the verdict is `Pass` at confidence ≥ ship-gate threshold AND every claim in the agent's response reconciles against the SHA chain AND no high-severity Q4 concerns are above threshold, the agent's "done" signal is trusted; otherwise, the system surfaces the specific gap.

**For an operator monitoring the system:**

1. A weekly evaluation report shows `prediction_oracle_correlation` per Python mutation category on rolling-window holdout against the real 300×8 corpus, with Python-stratified conformal coverage and OOD AUC.
2. Active-learning queue surfaces the 30 most uncertain real-world patches for Docker oracle re-run.
3. Drift detector flags any Python cell conformal-coverage regression > 5% over the last 1,000 attempts.
4. Self-healing scheduler runs A/B/C promotion automatically when drift is detected; operator approves only the catastrophic-drift retrain.
5. Storage hygiene runs nightly GC, demoting cool panels to lower tier compression, evicting beyond-quota.

**For the project as a whole:**

The line in the sand is: when ME-JEPA's per-Python-mutation-category correlation is stable at >= 0.95 over four consecutive rolling windows on the full 300×8 SWE-bench Lite Python corpus, **the system is the verifier for Python AI-generated patches**. Code-review-as-a-gate becomes statistically unjustified for any Python patch that ME-JEPA gives `Verdict::Pass` to with confidence above the ship-gate threshold. That doesn't eliminate the role of senior engineers — it shifts them from "did the AI do what it said" to "is the verifier itself still calibrated, and what does the active-learning queue say." Per `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §6`: the role becomes "occasional spot-checker of the verifier-of-the-verifier."

## 7. What this is not

- **Not a replacement for intent.** The agent (and the operator behind it) still says what they want done. ME-JEPA verifies the *result*.
- **Not a replacement for tests.** Tests are part of the truth signal; ME-JEPA predicts what tests would say. When ME-JEPA is wrong, the test outcome is the ground truth that retrains it.
- **Not a hallucination detector for natural language.** ME-JEPA cares about bytes on disk and oracle outcomes, not about whether the agent's prose is accurate. If the agent says "I think this is right" without editing anything, ME-JEPA returns "no shift, no prediction needed."
- **Not permanently language-restricted.** The architecture is language-agnostic, but the active ship path is Python. Per-language operators are a later operational extension after the Python truth system is green.
- **Not domain-restricted.** ME-JEPA-Code is the code-domain instance of TCT; ME-JEPA-Voice / ME-JEPA-Image / ME-JEPA-X follow the same template (frozen instruments + predictor + Gτ guard + observe/EWC++/A/B/C).

## 8. Falsifiable predictions about the end state

Per the operator's stated invariant ("system is built to improve continuously without external intervention"):

| Prediction | Falsifier |
|---|---|
| `prediction_oracle_correlation` >= 0.95 stable over 4 consecutive rolling windows on 300×8 SWE-bench Lite Python | Any window drops below 0.92 without a coincident known infrastructure incident |
| Python-cell conformal coverage stays within 88-92% of stated 90% ± 2% on holdout for each Python mutation category | Any Python mutation category exceeds ±5% deviation for two consecutive evaluation cycles |
| OOD AUC ≥ 0.85 between in-distribution and synthetic OOD inputs | Any week below 0.80 |
| Subscriber p50 latency < 5s, p99 < 30s under production traffic (every Edit shift → panel materialized → ME-JEPA prediction available) | Two weeks at p99 ≥ 60s |
| Self-healing detects soft drift (88-85%) within 1,000 observations and promotes mode-winner without operator intervention | Operator forced to manually trigger A/B/C in any non-catastrophic case |
| Disk footprint stable at ≤ 110 GB under nightly GC with 115 GB hard quota | Quota exceeded for > 24h without emergency eviction kicking in |
| Inner-LLM residue absent from build surface (no `llama-cpp-2`, no Qwen pinning, no Mode B drip) | Any commit reintroduces these without explicit operator override |

These are the public commitments of the futurebuild plan. If any of them fails sustainably, the architecture is wrong and we should revise — not paper over.
