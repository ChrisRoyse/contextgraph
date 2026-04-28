# Shakespeare TCT Plan

This directory is the planning package for the next Shakespeare LoRA run.

Goal: train a model that can answer any user request using Shakespeare's writing style without merely reciting Shakespeare's words, refusing modern topics, echoing prompts, or collapsing into archaic-token loops.

The plan assumes Context Graph is no longer a 13-embedder system only. E14 BGE-M3 Dense is now present in the repo and should be used primarily for multilingual and long-context content preservation, not as a style signal by default.

## Recommended Decision

Do not publish or build further on any v4.2 artifact until its training data and production-stack FSV are audited. The current workspace contains v4.2 CAMD/DPO artifacts, but sampled CAMD records still include scene headers, stage directions, and unrelated Context Graph implementation notes. That is exactly the class of contamination that caused earlier regressions.

The next serious run should be a clean `v5.0-tct` run:

1. Start from v2.1 as the stable adapter baseline.
2. Rebuild the training data with strict contamination gates.
3. Use Teleological Constellation Training as the shared primitive for data labelling, DPO pair mining, training weights, runtime guard, and FSV.
4. Train conservatively with SFT refresh plus CAMD-DPO, not broad high-LR continue-training.
5. Ship only after production-stack Playwright FSV passes, not just direct `model.generate()` tests.

## Documents

| File | Purpose |
|---|---|
| `00_current_state_review.md` | What has already happened, what worked, what failed, and what is unsafe to reuse. |
| `01_target_behavior.md` | The behavioral contract for "talking to Shakespeare" without topic refusal or semantic copying. |
| `02_research_notes.md` | Primary-source research notes and how they map onto this project. |
| `03_tct_architecture.md` | The optimized Teleological Constellation Training architecture for text style. |
| `04_data_pipeline.md` | Dataset construction, cleaning, contamination gates, and pair-mining rules. |
| `05_training_recipe.md` | Concrete v5 training recipe, hyperparameters, and checkpoints. |
| `06_runtime_guard.md` | Multi-view guard, constrained decoding, structured-output handling, and anti-loop checks. |
| `07_evaluation_fsv.md` | Full State Verification protocol and hard publish gates. |
| `08_execution_plan.md` | Ordered implementation plan with dependencies and expected outputs. |
| `09_risk_register.md` | Risks, failure modes, mitigations, and stop conditions. |
| `10_derived_data_superboost.md` | How to exploit the 249k+ derived signals without synthetic-data collapse. |

## Pre-Training FSV Harness

Run this before any long training job:

```bash
python3 scripts/shakespeare_v5_tct_preflight_fsv.py
```

It writes the physical source of truth for the smoke gate under:

```text
docs/shakespeareplan/runs/<run_id>/
```

The run directory contains a SQLite evidence database, copied smoke SFT/DPO
files made only from existing real records that pass hard gates, edge-case
fixtures, and a reopened-state report. Synthetic fixtures are used only to test
failure handling and are never written to `~/.contextgraph/training`.

## North Star

The model should behave as a modern knowledgeable assistant with Shakespeare's style prior:

- Style is invariant.
- Topic is unrestricted.
- Content is accurate.
- Outputs are not memorized passages.
- Runtime has deterministic verification relative to the frozen constellation panel.
