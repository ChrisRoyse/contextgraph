# futurebuild — ME-JEPA as the Reality Truth System

> **Purpose.** This directory is the complete forward-looking plan for turning ME-JEPA-Code into the operational truth system that predicts, in real time, whether code an AI agent has produced will work in reality. It supersedes the historical reality-loop framing (outer Claude + inner Qwen LLM) and consolidates everything around a **single outer loop around Claude Code**, whose only job is to generate the durable reality stream that ME-JEPA trains on and serves predictions about.
>
> **MANDATORY first read for any agent picking up a TASK-* file:** [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md). The protocol defines the inherited verification contract — SoT readback, ≥3 boundary cases, NO mocks, NO backwards compatibility, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts. A task is not done without these, regardless of `cargo test` results. **`ASSUME THE TASK IS WRONG`** — audit every path, symbol, and CF reference against the live tree (FSV-PROTOCOL §7).
>
> **Authority.** When this suite conflicts with `docs/ruvectorfindings/specs/_README.md` or `.claude/loop.md`, the change is intentional — apply it. The operator approved the pivot in the prompt that originated this directory. `CLAUDE.md` is unchanged; the rules in §11 still apply, but the *content* of the work has shifted from "build ME-JEPA primitives" (largely done) to "operationalize ME-JEPA as the reality truth system that AI agents query."

**Date authored:** 2026-05-10
**Date revised:** 2026-05-13 — Phase E self-optimization policy code/FSV shipped; Phase B code/FSV status reconciled; Phase C corpus expansion completed and verified; **Phase F production-observability + runbook + DR + API is code/FSV complete** with all Phase F GitHub implementation issues closed.
**Independent audit:** 2026-05-12 — verified Phase A–D against the live code tree and `/mnt/d/contextgraph/fsv/` evidence files. 35/35 items PRESENT, 0 MISSING, 0 PARTIAL. All 17 filed GitHub issues (#3–#19) including the umbrella readiness gate [#15](https://github.com/ChrisRoyse/contextgraph/issues/15) are CLOSED. See `./memory/discoveries/2026-05-12--phase-a-d-independent-audit-zero-gaps.md` for the audit trail and `docs/MEJEPA_SUPER_PROMPT.md` for the consolidated agent operating contract.
**Status:** active blueprint — Phase A, Phase B prediction/API/schema, Phase C corpus expansion, the Phase D enrichment substrate, Phase E self-optimization policy, and Phase F production operations are all code/FSV-shipped. Phase A-D are independently audit-verified; Phase E evidence is `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`; the Phase F evidence index is enumerated in `13_TASK_INDEX.md` and includes weekly dashboard, runbook CLI, storage restore/migrate, renderer, health-probe, deprecation, toolchain, cross-language, operator-override, pause, system-cost, restart-safety, multi-agent, catastrophic-event, storage-exhaustion, chaos, load, soak, adversarial-corpus, retention, witness-quarantine, workflow-pin, and agent-identity FSV artifacts. There are no open Phase F implementation issues; `phase:f-followup` is empty as of 2026-05-13. The latest holdout still reports `prediction_oracle_correlation = 0.866667 < 0.95`, so human-review replacement remains a Phase G convergence target, not a completed claim and not a Phase A–F implementation gap. Phase H tracks remaining inner-loop residue cleanup only.
**Scope:** absorbs `09`, `10`, `11`, `12`, `13`, `14`, `15`, `16` from `docs/ruvectorfindings/`; refactors `.claude/loop.md`; deletes the inner-loop residue per the operator directive.

---

## North star invariant

ME-JEPA-Code is the product. LLM agents are untrusted edit producers; hooks, MCP tools, AST chunks, corpora, embedders, DDA, TCT, telemetry, and docs exist only to train, calibrate, serve, or audit ME-JEPA's `RealityPrediction`.

Every futurebuild task must satisfy at least one of these roles:

1. capture a durable reality event from bytes on disk, tool output, oracle result, operator feedback, or agent surprise;
2. transform that event into a frozen-instrument target, DDA signal, TCT constellation cell, calibration window, or training certificate;
3. train, regularize, calibrate, evaluate, or promote the ME-JEPA predictor;
4. serve the Q1-Q5 `RealityPrediction` surface to an AI agent or operator;
5. feed prediction error, drift, or surprise back into replay, active learning, or self-healing.

Anything outside those roles is either support tooling with a named source-of-truth readback, or it should be cut or tracked as a design exception.

## Reading order

| # | Document | What it covers |
|---|---|---|
| 1 | [`01_VISION_TRUTH_SYSTEM.md`](01_VISION_TRUTH_SYSTEM.md) | The end goal: ME-JEPA replaces human code review by predicting reality |
| 2 | [`02_CURRENT_STATE_HONEST.md`](02_CURRENT_STATE_HONEST.md) | What's actually shipped vs aspirational — no marketing |
| 3 | [`03_ARCHITECTURE_BLUEPRINT.md`](03_ARCHITECTURE_BLUEPRINT.md) | Reference architecture: edits → embeddings → panels → ME-JEPA → verdicts → agents |
| 4 | [`04_DDA_TCT_CODE_ENRICHMENT.md`](04_DDA_TCT_CODE_ENRICHMENT.md) | The multi-embedder labeling pipeline — DDA explodes code into N·(N+N²) signals |
| 5 | [`05_EMBEDDER_SIGNAL_MAP.md`](05_EMBEDDER_SIGNAL_MAP.md) | Which of the 21 embedders carry signal for code-reality prediction; routing table |
| 6 | [`06_REALITY_LOOP_REFACTOR.md`](06_REALITY_LOOP_REFACTOR.md) | The single outer-loop-around-Claude-Code design replacing the bipartite Mode A/B |
| 7 | [`07_TRAINING_DATA_FLYWHEEL.md`](07_TRAINING_DATA_FLYWHEEL.md) | How every Edit shift → panel → corpus row; replay-buffer + prioritized sampling |
| 8 | [`08_LIVE_PREDICTION_AGENT_API.md`](08_LIVE_PREDICTION_AGENT_API.md) | The MCP tool surface AI agents call to ask "will this work?" + injection-back |
| 9 | [`09_SELF_HEALING_AUTONOMY.md`](09_SELF_HEALING_AUTONOMY.md) | Autonomous learning: observe + EWC++ + drift + A/B/C, no human in the loop |
| 10 | [`10_LEARNING_FORMULA_MEJEPA_TRUTH.md`](10_LEARNING_FORMULA_MEJEPA_TRUTH.md) | UTML v1.0 (JEPA Edition) operationalized for code-reality prediction |
| 11 | [`11_DELETION_LIST_INNER_LOOP.md`](11_DELETION_LIST_INNER_LOOP.md) | What to delete from the codebase: inner-LLM residue, Mode B drip, etc. |
| 12 | [`12_IMPLEMENTATION_PHASES.md`](12_IMPLEMENTATION_PHASES.md) | Phased rollout with effort estimates and ship gates |
| 13 | [`13_TASK_INDEX.md`](13_TASK_INDEX.md) | Atomic task graph (DAG) by domain with dependencies |
| 14 | [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md) | The refactored `.claude/loop.md` content (single outer loop around Claude Code) |
| 15 | [`15_PREDICTION_TAXONOMY.md`](15_PREDICTION_TAXONOMY.md) | Complete catalog of every prediction ME-JEPA emits (Q1–Q5, all heads, all classes) |
| 16 | [`16_STORAGE_LOCATION_POLICY.md`](16_STORAGE_LOCATION_POLICY.md) | D-drive only for bulk state — locked decision restated for futurebuild |
| 17 | [`17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md`](17_BLIND_SPOTS_AND_MISSING_CAPABILITIES.md) | Audit of what 00–16 missed; resolutions added in 18–26 |
| 18 | [`18_INTEGRATION_AND_E2E_TESTING.md`](18_INTEGRATION_AND_E2E_TESTING.md) | Integration / E2E / chaos / load / soak testing strategy |
| 19 | [`19_OPERATOR_RUNBOOK.md`](19_OPERATOR_RUNBOOK.md) | Daily / weekly / monthly / incident operations |
| 20 | [`20_DISASTER_RECOVERY_AND_BACKUP.md`](20_DISASTER_RECOVERY_AND_BACKUP.md) | Backup strategy + restore procedures + DR drill |
| 21 | [`21_COLD_START_AND_BOOTSTRAP.md`](21_COLD_START_AND_BOOTSTRAP.md) | Fresh-deploy behavior + baseline checkpoint + cold-start widening |
| 22 | [`22_ADVERSARIAL_ROBUSTNESS.md`](22_ADVERSARIAL_ROBUSTNESS.md) | Adversarial threats + defenses; secret redaction; cross-language label gating |
| 23 | [`23_MULTI_AGENT_ISOLATION.md`](23_MULTI_AGENT_ISOLATION.md) | Multi-agent / multi-tenant isolation + concurrency |
| 24 | [`24_API_STABILITY.md`](24_API_STABILITY.md) | Semver / deprecation cycle / replay reproducibility |
| 25 | [`25_FUTURE_DOMAINS_MEJEPA_X.md`](25_FUTURE_DOMAINS_MEJEPA_X.md) | Template for ME-JEPA-Voice / -Image / new modalities |
| 26 | [`26_DOCUMENTATION_MAP.md`](26_DOCUMENTATION_MAP.md) | Cross-repo navigation + glossary |

## Spec hierarchy

| Level | Where | Purpose |
|---|---|---|
| 0 — Verification | [`specs/FSV-PROTOCOL.md`](specs/FSV-PROTOCOL.md) | **mandatory inherited contract** for every task: SoT readback, boundary audit, no mocks, fail-closed errors |
| 1 — Constitution | inherits from `docs/ruvectorfindings/specs/constitution.md` | immutable rules |
| 2 — Functional | [`specs/functional/`](specs/functional/) | what to build, per domain |
| 3 — Technical | [`specs/technical/`](specs/technical/) | how to build, per domain |
| 4 — Tasks | [`tasks/`](tasks/) | atomic work units, sequenced, with definition of done |

## Domain map

| Domain | Functional spec | Technical spec | Tasks dir |
|---|---|---|---|
| **FLYWHEEL** — turn Claude Code's Edit shifts into ME-JEPA training data continuously | [`SPEC-FLYWHEEL.md`](specs/functional/SPEC-FLYWHEEL.md) | [`TECH-FLYWHEEL.md`](specs/technical/TECH-FLYWHEEL.md) | [`tasks/flywheel/`](tasks/flywheel/) |
| **PREDICT** — agent-facing live prediction API (MCP tool surface) + verdict injection | [`SPEC-PREDICT.md`](specs/functional/SPEC-PREDICT.md) | [`TECH-PREDICT.md`](specs/technical/TECH-PREDICT.md) | [`tasks/predict/`](tasks/predict/) |
| **ENRICHMENT** — DDA/TCT multi-embedder explosion of code into 462+ signals per chunk | [`SPEC-ENRICHMENT.md`](specs/functional/SPEC-ENRICHMENT.md) | [`TECH-ENRICHMENT.md`](specs/technical/TECH-ENRICHMENT.md) | [`tasks/enrichment/`](tasks/enrichment/) |
| **LOOP** — refactor `.claude/loop.md` + delete inner-loop residue | [`SPEC-LOOP.md`](specs/functional/SPEC-LOOP.md) | [`TECH-LOOP.md`](specs/technical/TECH-LOOP.md) | [`tasks/loop/`](tasks/loop/) |
| **HEAL** — autonomous observe + EWC++ + drift + A/B/C scheduler | [`SPEC-HEAL.md`](specs/functional/SPEC-HEAL.md) | [`TECH-HEAL.md`](specs/technical/TECH-HEAL.md) | [`tasks/heal/`](tasks/heal/) |
| **CORPUS** — completed 300×8 SWE-bench Lite Python corpus + 10-language fixture corpus with real toolchain validation | (covered by existing [`SPEC-CORPUS-functional.md`](../ruvectorfindings/specs/functional/SPEC-CORPUS-functional.md)) | (covered) | [`tasks/corpus/`](tasks/corpus/) |
| **OBSERVABILITY** — live conformal/OOD/p50/p99 capture; ship-gate dashboard | [`SPEC-OBSERVABILITY.md`](specs/functional/SPEC-OBSERVABILITY.md) | [`TECH-OBSERVABILITY.md`](specs/technical/TECH-OBSERVABILITY.md) | [`tasks/observability/`](tasks/observability/) |
| **TESTING** — integration / E2E / chaos / load / soak harnesses | [`SPEC-TESTING.md`](specs/functional/SPEC-TESTING.md) | [`TECH-TESTING.md`](specs/technical/TECH-TESTING.md) | [`tasks/testing/`](tasks/testing/) |
| **RUNBOOK** — operator daily/weekly/incident operations + CLI | [`SPEC-RUNBOOK.md`](specs/functional/SPEC-RUNBOOK.md) | [`TECH-RUNBOOK.md`](specs/technical/TECH-RUNBOOK.md) | [`tasks/runbook/`](tasks/runbook/) |
| **DR** — disaster recovery + backup + restore + drill | [`SPEC-DR.md`](specs/functional/SPEC-DR.md) | [`TECH-DR.md`](specs/technical/TECH-DR.md) | [`tasks/dr/`](tasks/dr/) |
| **SECURITY** — adversarial robustness + secret redaction + audit trails | [`SPEC-SECURITY.md`](specs/functional/SPEC-SECURITY.md) | [`TECH-SECURITY.md`](specs/technical/TECH-SECURITY.md) | [`tasks/security/`](tasks/security/) |
| **BOOTSTRAP** — fresh-deploy + baseline checkpoint + cold-start widening | (in [`21_COLD_START_AND_BOOTSTRAP.md`](21_COLD_START_AND_BOOTSTRAP.md)) | (in `21 §3-7`) | [`tasks/bootstrap/`](tasks/bootstrap/) |
| **API** — semver / deprecation / replay / migration | (in [`24_API_STABILITY.md`](24_API_STABILITY.md)) | (in `24`) | [`tasks/api/`](tasks/api/) |

## What this directory is NOT

- It is not a re-derivation of UTML v1.0 — that lives in `docs2/TheoryOfMachineLearning.md` and `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`. We *use* it, we don't re-prove it.
- It is not a re-derivation of TCT — that lives in `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`. We *consume* it as the predictor-side collapse defense and the universal-agent-verifier surface.
- It is not a Phase 0–8 rebuild plan — most of those phases are SHIPPED. This directory documents the **operational integration** layer that turns the shipped primitives into a self-running truth system.
- It is not a re-architecture — the architecture is correct. This is the productionization plan.

## Cross-references

- Master plan that this absorbs: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md`
- Pivot statement that this respects: `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md`
- Learning formula (use as-is): `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md`
- Embedder doctrine (extended in `05`): `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md`
- TCT (consumed as collapse defense): `docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md`
- AST chunking (consumed by enrichment): `docs/ruvectorfindings/14_AST_AWARE_CHUNKING_FOR_EMBEDDERS.md`
- Hardening evidence ledgers: `docs/ruvectorfindings/{15,16}_PHASE_*_HARDENING_FSV_*.md`
- Operator role contract: `CLAUDE.md §11`
- The (about-to-be-replaced) loop spec: `.claude/loop.md` — see [`14_NEW_LOOP_MD.md`](14_NEW_LOOP_MD.md) for the replacement
