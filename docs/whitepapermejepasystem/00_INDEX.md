# ME-JEPA-Code Whitepaper

**Date:** 2026-05-15
**Status:** Draft v1 — written while the seed-corpus forward-cache run is producing on aiwonder
**Audience:** Operator, engineers, fresh AI agents picking up the work, future paper reviewers

> **The single sentence.** ME-JEPA-Code is the deterministic predictive truth system that lets a software engineer stop reviewing AI-generated code, because the system itself can answer — to calibrated, auditable certainty — whether what the AI claimed exists, whether it works, why it would fail, what else could go wrong, and how it changes reality.

---

## Reading order

This is one whitepaper split across 14 files so each part can be read, edited, and forensically audited independently. If you only have ten minutes, read `01_EXECUTIVE_SUMMARY.md`. If you only have an hour, read `01` then `04` (Five Questions) then `09` (Training Loop).

| # | File | What it answers | Read first if you care about |
|---|---|---|---|
| 00 | [`00_INDEX.md`](00_INDEX.md) | This map | Navigation |
| 01 | [`01_EXECUTIVE_SUMMARY.md`](01_EXECUTIVE_SUMMARY.md) | What ME-JEPA-Code is, in one page | Anyone |
| 02 | [`02_THE_PROBLEM.md`](02_THE_PROBLEM.md) | Why AI-generated code review is broken today | Engineering management |
| 03 | [`03_ARCHITECTURE.md`](03_ARCHITECTURE.md) | The system, end to end | System architects |
| 04 | [`04_FIVE_OPERATOR_QUESTIONS.md`](04_FIVE_OPERATOR_QUESTIONS.md) | Q1 EXIST, Q2 WORK, Q3 WHY, Q4 WHAT ELSE, Q5 IMPACT | Anyone who wants the user-visible contract |
| 05 | [`05_FROZEN_INSTRUMENT_PANEL.md`](05_FROZEN_INSTRUMENT_PANEL.md) | The 15-slot target panel + collapse-immunity proof | ML researchers, JEPA specialists |
| 06 | [`06_EMBEDDER_STACK.md`](06_EMBEDDER_STACK.md) | The 12 active embedders + asymmetric variants | Embedding engineers |
| 07 | [`07_TELEOLOGICAL_CONSTELLATIONS.md`](07_TELEOLOGICAL_CONSTELLATIONS.md) | Cells, reward signals, 462-signal-per-chunk math | Reward-signal engineers |
| 08 | [`08_TRAINING_CORPUS.md`](08_TRAINING_CORPUS.md) | The 300×8 SWE-bench Lite corpus + mutation operators | Data engineers |
| 09 | [`09_TRAINING_LOOP.md`](09_TRAINING_LOOP.md) | JEPA learning formula + UTML + online retrain | ML training |
| 10 | [`10_LIVE_FEEDBACK_LOOP.md`](10_LIVE_FEEDBACK_LOOP.md) | Hooks + MCP + agent-feedback ingestion | Agent integrators |
| 11 | [`11_SHIP_GATE.md`](11_SHIP_GATE.md) | Convergence criterion: 0.95 oracle correlation × 4 windows | Anyone tracking progress |
| 12 | [`12_FUTURE_CAPABILITIES.md`](12_FUTURE_CAPABILITIES.md) | What the corpus unlocks beyond pass/fail prediction | Operator, researchers |
| 13 | [`13_GLOSSARY.md`](13_GLOSSARY.md) | Every term defined once | Everyone |

---

## Relation to the existing futurebuild planning

This whitepaper is the **publication-form summary** of the planning materials under [`docs/futurebuild/`](../futurebuild/) and [`docs/ruvectorfindings/`](../ruvectorfindings/). The planning docs are the operational source of truth (task indices, FSV protocols, deletion lists, phase plans). The whitepaper is what you hand to someone who is not yet inside the project. When the two disagree on operational detail, the planning docs win.

**Anchor docs you should read next** (in this order, after the whitepaper):
1. [`docs/futurebuild/01_VISION_TRUTH_SYSTEM.md`](../futurebuild/01_VISION_TRUTH_SYSTEM.md) — original vision statement
2. [`docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md`](../futurebuild/03_ARCHITECTURE_BLUEPRINT.md) — fuller architecture
3. [`docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`](../futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md) — the learning math
4. [`docs/futurebuild/15_PREDICTION_TAXONOMY.md`](../futurebuild/15_PREDICTION_TAXONOMY.md) — the prediction-head taxonomy
5. [`docs/futurebuild/27_PHASE_G_ROADMAP.md`](../futurebuild/27_PHASE_G_ROADMAP.md) — the current convergence sprint
6. [`docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`](../ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md) — collapse-immunity derivation
7. [`docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`](../ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md) — reward signal math
8. [`CLAUDE.md`](../../CLAUDE.md) — project addendum for agents
9. [`docs2/AICodingAgentSuperPrompt.md`](../../docs2/AICodingAgentSuperPrompt.md) — master doctrine

---

## What this whitepaper is NOT

- **Not a defense of LLMs.** The LLM is not part of ME-JEPA. The LLM is the untrusted edit generator that ME-JEPA verifies.
- **Not a paper about chat assistants.** This is about *verifying* AI-generated code, not generating it.
- **Not an architecture review of one neural model.** ME-JEPA is a *system* — predictor + target panel + reward field + corpus + hooks + MCP — not a single model.
- **Not "AI safety" in the existential sense.** This is "AI honesty in software engineering": did what the AI said actually happen?
- **Not benchmarks vs other systems.** The benchmark is `prediction_oracle_correlation ≥ 0.95` on a real Docker-evaluated corpus — pass or fail.

---

## One paragraph of project state at writing time

The system is **mid-construction**. The seed training data is being produced **right now** on the aiwonder datacenter PC (RTX 5090, AMD Ryzen 9 9950X). The forward-cache run is processing the full **300 SWE-bench Lite Python tasks × 8 mutation categories × 12 embedders ≈ 4.9 million vector records**, currently at ~38 of 391 shards complete, ETA ~20 hours. Once the seed corpus is done, the next phase produces the frozen-target instrument panel, runs the JEPA training loop, hits the 0.95 oracle-correlation gate per (mutation_category × language) cell, and ships the MCP server that AI coding agents call. This whitepaper is the operator-and-engineer-facing description of what's being built and why.
