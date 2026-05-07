---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:02:00Z
status: active
severity: low
tags: exp2, corpus, sampling, determinism, seed
---

# Defect: sample_sources uses seed=42 for ALL strata, not per-stratum seeds; manifest claims selection_seed=42

## Where
`scripts/exp2/build_corpus.py:277-283` (`sample_sources`) and line 305 (manifest `selection_seed: 42`).

## Spec
`04_data_corpus.md` §3 (manifest example):
```
"S1_shakespeare": {"selection_seed": 42},
"S2_code": {"selection_seed": 43},
```

## Observed
- `sample_sources(seed=42)` at line 350 in `main()` is called once with seed 42.
- Inside the loop (lines 279-283), a *new* `random.Random(seed)` is constructed **inside** the per-stratum loop with the same seed=42 for every stratum. So every stratum gets the same rng state at start.
- The manifest at line 305 hardcodes `selection_seed: 42` for all strata. Inconsistent with the spec example which suggested per-stratum seed 42, 43, ....

## Why it matters
- Determinism (C6) is preserved (same seed → same sample), so this is not a fail-loud violation.
- But the spec's intent of per-stratum seeds (preventing trivial interleaved sample correlations) is lost.
- Minor: `build_pca_training` (line 354) is called with `seed=43`, which IS consistent with `04_data_corpus.md` §9 (PCA seed = 43).

## Severity
LOW. The actual numeric outputs are deterministic. The spec example was prescriptive in shape but the implementation chose seed=42 across all strata. Should be:
- Either fix `sample_sources` to use per-stratum seeds, OR
- Update the manifest schema in `04_data_corpus.md` §3 to clarify that a single seed=42 is used.

## Cross-check
Agent-02: run sample_sources twice with the same input strata files and confirm identical outputs.
