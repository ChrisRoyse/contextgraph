# EXP-2 Lessons

## 2026-04-28 — Environment Verification Must Enforce Free VRAM

What happened: the first T-001 verifier checked that an RTX 5090 existed but did not enforce the required 30 GB free VRAM. That allowed a false pass while `nvidia-smi` showed only 10,730 MiB free.

Why it matters: EXP-2 embedding extraction and InfoNCE training require the real 14-embedder GPU path. A stale binary and installed models are not sufficient proof that the run can start.

Fix: `scripts/exp2/verify_environment.py` now parses `nvidia-smi` and fails unless an RTX 5090 has at least 30,000 MiB free. The verifier writes `verification_log.json` even on failure.

Prevention rule: every environment acceptance criterion in `docs/exp2/06_atomic_tasks.md` must be asserted directly, not inferred from a weaker adjacent signal.

## 2026-04-28 — KSG Bootstrap Needs Explicit Duplicate Handling

What happened: corpus bootstrap resampling creates duplicate rows by design, but KSG on continuous variables has zero-radius ties when duplicate samples are passed directly.

Why it matters: silently clamping or ignoring those resamples would make CI values untrustworthy.

Fix: production KSG still fails on duplicate raw data. Bootstrap-only KSG uses deterministic `1e-12` tie-break jitter and records that duplicate policy in each pair result.

Prevention rule: estimator numerical policies that change the data, even at tiny scale, must be explicit in artefacts and not hidden inside helper functions.

## 2026-04-29 — Partial EXP-2 Artefacts Must Not Be Certifiable

What happened: the workspace contained a 600-input `corpus_manifest_v1.json` that excluded S4 learning-events and S8 multilingual strata, while the EXP-2 contract requires 800 inputs across 8 strata.

Why it matters: downstream estimator, audit, and FSV phases could otherwise operate on an invalid corpus and make the run look complete.

Fix: production corpus construction now rejects excluded strata, extraction checks exact manifest counts and no deviations, audit finalization refuses missing phase chains or artefacts, and FSV requires production artefacts plus a fresh replay.

Prevention rule: missing or partial EXP-2 state is never recorded as a successful artefact; it must raise with the exact missing source of truth.
