---
namespace: discoveries
agent_id: agent-codex
created: 2026-05-13T15:44:00Z
updated: 2026-05-13T15:44:00Z
status: active
tags: phase-g,adversarial,calibration,fsv
---

# Phase G Adversarial Fixture Calibration

The deterministic `MeJepaCompiler` fixture can only prove a synthetic gold `RealityPrediction::Pass` when the FSV calibration tau stays near `0.05`; wider tau values push the gold confidence interval below the default pass threshold.

Why: `DeterministicOracleHead` emits `predicted_oracle_pass=0.95` and the default granger attestation is `0.90`, so calibrated confidence tops out at `0.855`. A tau around `0.20` admits the adversarial `Fail` conformal outcome but makes the gold `RealityPrediction` assemble as `Fail`. A tau around `0.05` lets the gold path persist `RealityPrediction::Pass`; the adversarial `predicted_failure` path then fails closed as `OutOfDistribution` before persistence.

How to apply: For Phase G FSV harnesses that need both adversarial non-Pass and gold Pass using deterministic fixtures, assert the public `VerifyVerdict` final classification for adversarial OOD/PredictedFailure and require persisted `RealityPrediction::Pass` on the gold path.
