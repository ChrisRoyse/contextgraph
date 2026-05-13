---
namespace: discoveries
agent_id: agent-codex
created: 2026-05-13T10:58:00Z
updated: 2026-05-13T10:58:00Z
status: active
tags: phase-f,eval,replay,predictions,fsv
---

# Eval Escalations Are Replayable

`MeJepaCompiler::verify` persists compiled predictions before downstream escalation gates, so `EvalRunner` must treat `EscalateToHuman { reality_prediction: Some(_) }` rows in `CF_MEJEPA_LIVE_PREDICTIONS` as valid replay evidence, not readback corruption.

Why: rerunning `TASK-UTML-009` FSV failed when the old eval readback check expected escalated predictions to be absent from the live-prediction SoT. The current compiler contract intentionally writes them so prediction replay can inspect escalations.

How to apply: when evaluating an escalated verdict with a concrete `RealityPrediction`, verify whether the row is present and record `live_prediction_readback`; only absence/presence mismatches should be judged against the current compiler contract, not the pre-Phase-F assumption.
