# 01 — Executive Summary

## What it is

**ME-JEPA-Code** is a deterministic, calibrated, multi-head predictor that takes an AI-generated code change and returns a **`RealityPrediction`** answering five questions before the code is ever executed:

| # | Question | What the head returns |
|---|---|---|
| **Q1** | Does what the AI claimed **EXIST** on disk? | Claim graph reconciled against the SHA chain of edited bytes |
| **Q2** | Does it **WORK**? | `Verdict ∈ {Pass, Fail, OutOfDistribution, Abstain, GuardRejected}` + calibrated conformal interval |
| **Q3** | **WHY** would it fail? | Ranked `predicted_failure_modes` + predicted test flips + violating embedders + nearest-neighbor exemplars |
| **Q4** | **WHAT ELSE** could go wrong? | Always-on heads for edge cases, latent bugs, tech debt, dead code, redundancy, perf, security, accuracy, cost |
| **Q5** | How does it **IMPACT REALITY**? | Shift log replayed against predicted side-effects → `RealityImpact` with blast-radius scoring |

ME-JEPA-Code runs as a **local MCP server** on the operator's hardware. AI coding agents (Claude Code, Cursor, Copilot, Devin, etc.) call it via JSON-RPC stdio for predictions and to feed back the outcomes when those predictions are wrong. Underneath, a continuous self-optimization daemon retrains the predictor online from every Edit shift, every test outcome, and every operator override.

The LLM is not part of the verifier. It is an **untrusted edit generator** whose claims ME-JEPA accepts, rejects, or learns from. This separation is the whole point.

## What it solves

Every AI coding agent today emits patches accompanied by natural-language assertions: *"I fixed the bug. The tests pass. This is ready to ship."* Those assertions are **socially fluent and epistemically empty**. Nothing in the agent's output is bound to:

- the actual bytes-on-disk reality of the repository after the edit,
- the outcome of running the tests it claims to have passed,
- a verifier that the agent did not author.

The result is a quiet, expensive failure class: patches that look right but never apply, tests that pass locally and fail in CI, "fixed" issues that recur the same week, senior engineers reduced to humans-in-the-loop whose only job is to verify the assertions of a system that is faster than they are. The cost is a **tax on every line of AI-produced code**: someone must read it, run it, and decide whether the assertion was true. That someone is the bottleneck.

**ME-JEPA-Code's thesis: human review of AI-produced code is statistically unjustified the moment the system can predict, to calibrated certainty, what the deterministic reality oracle would say if the code were actually run.**

## How it works (one paragraph)

ME-JEPA is a **JEPA-class** (Joint-Embedding Predictive Architecture) system with one decisive architectural innovation: **structural collapse-immunity on the target side**. The target representation `f_target(panel)` is produced by a panel of **15 frozen, externally-defined instruments** (deterministic functions of typed events — AST chunks, Docker oracle outcomes, witness chain entries, type-graph features, data-flow features, oracle event taxonomies, etc.) with **zero trainable parameters**. By construction, `ΔΞ.target_collapse = 0` ([proof in §5.2 of `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`](../ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md)). On the predictor side, a stack of 12 active content embedders (E1 e5-large-v2 semantic, E2/E3/E4 algorithmic temporal, E6 sparse keyword, E7 Qodo-Embed 1.5B code, E8 graph, E9 HDC, E10 multimodal-e5, E12 ColBERT, E13 SPLADE, E14 BGE-M3) maps the pre-edit + claim + context into a 5,120-dimensional joint embedding. A trainable predictor head learns the mapping `f_predictor(panel_t1) → f_target(panel_t2)`. Every prediction comes with a conformal interval and a fail-closed `Verdict::Abstain` mode when out-of-distribution.

## The corpus that grounds it

The training corpus is the **300-task SWE-bench Lite Python benchmark cross-multiplied by 8 mutation categories**: `known_good` (the gold patch — should `Pass`), and seven adversarial mutations that should `Fail` (`subtle_flip`, `off_by_one`, `swap_variable`, `delete_test_call`, `wrong_file`, `over_engineer`, `compile_error`). Every (task × mutation) cell has a known oracle outcome from running the official SWE-bench Docker evaluator. That gives **2,400 labeled (instance, mutation) reality-shift events**, expanded to **399,777 AST chunks** when decomposed into the actual code units the predictor sees. At 12 embedders per chunk, that's **~4.9 million vector records** of training substrate, all derivable from durable reality.

## The convergence criterion

ME-JEPA is "done" when the **`prediction_oracle_correlation ≥ 0.95`** metric is stable across **four consecutive rolling windows**, **stratified per `(mutation_category × language)` cell**, on the 300×8 Python corpus. Per-cell stratification ensures the system isn't just learning the easy `known_good` cell while failing on the adversarial mutations that matter. Conformal calibration ensures the predictor's uncertainty is honest. Once the four-window stability assertion fires green, the verifier *is* the system for Python AI-generated patches, and human code review is statistically reducible to spot-checking the verifier-of-the-verifier.

## The capability beyond pass/fail

Pass/fail is only the entry point. The labeled corpus + 12-embedder cross-attention + 15-instrument panel give ME-JEPA the substrate to extract reward signals operators couldn't surface manually. Speculatively (some confirmed, some hypotheses) the system can learn:

1. **Failure-shape fingerprints** — patches that look unrelated lexically share failure geometry across embedders (bad API assumptions, stale imports, type contract mismatch, state mutation in wrong layer, exception swallowing). ME-JEPA learns *"this patch shape tends to fail reality before tests run."*
2. **Claim-to-disk existence prediction** — for "I added validation to X" claims, ME-JEPA compares claimed intent, changed AST nodes, file-level witness chains, and context embeddings to predict whether the claim is in the bytes or only adjacent surface area.
3. **Oracle outcome prediction before execution** — the core "will it work in reality?" head, anchored to Docker oracle truth.
4. **Hidden reward from embedder disagreement** — when semantic says "close" but graph says "structurally different" and sparse sees API token drift, that disagreement becomes a high-risk signal.
5. **Repository-specific invariants** — which files co-change, which call paths are fragile, which AST shapes correlate with regression.
6. **Patch blast-radius prediction** — chunks whose embeddings shift relative to the patch even though they were not edited; surfaces hidden affected areas: tests, dependency callers, config paths, serialization contracts.
7. **Counterfactual repair ranking** — score candidate fixes before execution: *"this edit is more likely to satisfy the oracle than that edit."*
8. **Calibration of uncertainty** — out-of-distribution detection that emits lower confidence + points to missing evidence instead of overclaiming.
9. **Teleological constellations** — intent-linked chains: operator goal → agent claim → edit event → AST delta → embedded chunk constellation → frozen instruments → oracle outcome → feedback. The reward is the entire chain.
10. **Review-replacement primitives** — existence verification, work prediction, failure-reason prediction, edge-risk prediction, impact prediction, all tied to observed oracle outcomes.

(Full details in [`12_FUTURE_CAPABILITIES.md`](12_FUTURE_CAPABILITIES.md).)

## What's running right now

A continuous **forward-cache run** on the aiwonder datacenter PC (RTX 5090, 32 GB VRAM, AMD Ryzen 9 9950X, 32 threads, 121 GB RAM, ZFS hot pool 1.8 TB NVMe + archive pool 9.0 TB HDD mirror) is processing all 399,777 chunks through all 12 active content embedders. Mean throughput: **206 s/shard at par=6 parallelism** under CUDA fp16 with `--isolate-embedder-processes` (one subprocess per embedder, run concurrently with par=6 GPU bandwidth headroom). Total runtime ~21 hours. Output cache:

```
/zfs/archive/contextgraph/corpus/python-forward-cache-v1-aiwonder-prod-20260515-191116Z/
```

When that finishes, the seed corpus is ready for **Phase G**: train the JEPA predictor → conformal-calibrate → adversarial-evaluate → assert per-cell ship gate → rotate windows → declare ME-JEPA ready.

## Why this is solvable now

Three things have converged that didn't converge before:

1. **JEPA's collapse problem has a deterministic fix.** Frozen externally-defined instruments are not a model — they are functions that exist regardless of training. They can't collapse. This sidesteps the EMA / stop-gradient / VICReg jungle.
2. **Docker oracles are durable label sources.** SWE-bench Lite provides a per-task Docker container that gives an unambiguous Pass/Fail for any candidate patch. That replaces "did the LLM grader say yes?" with "did the bytes pass the tests in isolation?"
3. **Multi-embedder coverage is now affordable.** 12 embedders × 400 K chunks × fp16 on a single RTX 5090 fits in days, not months. The 2.94× speedup from #308 parallel-isolated mode put it inside a working week.

## What this isn't

Not a chat product. Not a code generator. Not a benchmark of LLMs against each other. Not an "AI safety" treatise. It is **a verifier** for the assertions AI coding agents make about code. It either predicts what reality will say, or it doesn't. The 0.95 oracle correlation gate is the only metric that matters.

## What success looks like

A senior engineer pushes their PR-review queue from 30/day to 0/day because ME-JEPA's `RealityPrediction` is calibrated enough that they trust its `Verdict::Pass` and only look at `Verdict::Fail` / `Abstain` / `OutOfDistribution` cases. Junior engineers stop reading every line of AI-generated patches. Project velocity goes up by 3-5× for the AI-code-heavy parts of the codebase. Reviewer fatigue collapses. Most importantly: when ME-JEPA is wrong, the operator submits one MCP `mejepa_record_agent_feedback` call and the predictor pulls that mistake into its replay buffer at 6× weight in the next training step — so it gets better the more it's used.

## Status at draft

- Seed corpus: **38 / 391 shards processed**, ETA ~20 h, healthy
- MCP tool surface: **28 live `mejepa_*` tools** in `context-graph-mcp` crate
- Frozen instrument panel: **15 slots defined, 5,120-d total**, implementation in `context-graph-mejepa-instruments`
- 12 active embedders: **wired, smoke-tested, parallel-isolated forward-cache verified**
- Phase G: **task index open, not yet started** (waits on seed corpus completion)
- Ship-gate: `prediction_oracle_correlation = 0.866667 < 0.95` (last measurement at the gate)
- All AIWONDER-* migration issues (#297–#308): **closed**
- All Phase A–D items (35/35): **PRESENT and FSV-audited** as of 2026-05-12

The road from here to 0.95 is concretely planned. Phase G item-by-item lives in [`docs/futurebuild/27_PHASE_G_ROADMAP.md`](../futurebuild/27_PHASE_G_ROADMAP.md).
