---
namespace: journal
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:16:30Z
status: active
severity: medium
tags: exp2, build-corpus, offline, strata, runtime-tested
---

# build_corpus.py: orchestrator phase=sources fails fast on S4; offline collectors S1/S2/S3/S7 individually pass

## Orchestrator level

```
python3 scripts/exp2/build_corpus.py --config scripts/exp2/exp2.yml --phase sources
EXP-2 corpus build failed: S4 requires --learning-events-json exported from real CF_LEARNING_EVENTS
```
Exit 1. Constitution C2 (fail-loud) satisfied — the orchestrator uses STRATA list `[S1, S2, S3, S4, S5, S6, S7, S8]` and dies at S4 because no `--learning-events-json` arg supplied.

## Collector-level individual sweep (offline)

I called each offline-friendly collector directly (bypassing the orchestrator) to gauge what each can produce on this box without internet:

| Stratum | Result | Need (MIN_CANDIDATES) | Verdict |
|---------|--------|------------------------|---------|
| S1 Shakespeare | 1611 candidates | ≥120 | OK (offline file at `shakespear/complete_works.txt` 5.4 MB) |
| S2 code | 3237 candidates | ≥120 | OK (scans `crates/`, `scripts/`, `config/`) |
| S3 causal claims | 767 candidates | ≥120 | OK (regex-grep on `docs/`, `tasks/`, `crates/` for causal markers) |
| S7 temporal sequence | 120 candidates | ≥120 | OK by exactly 1 (any future commits would push it over) |
| S4 learning events | needs --learning-events-json | n/a | requires manual export from CF_LEARNING_EVENTS |
| S5 BEIR | needs `datasets` + internet | ≥120 | offline: blocked |
| S6 Wikipedia | needs `datasets` + internet | ≥120 | offline: blocked |
| S8 multilingual FLORES | needs `datasets` + internet | ≥120 | offline: blocked |

## What this tells us about the corpus build readiness

- **4/8 strata** can be built offline (S1, S2, S3, S7) totaling 5,735 candidates.
- **3/8 strata** need internet (S5, S6, S8).
- **1/8 strata** needs an external export from a CG production CF (S4).

Per spec `04_data_corpus.md` the SAMPLE_PER_STRATUM is 100 (and there are 8 strata) → the harness wants 100 × 8 = 800 inputs total. Without internet, 4 strata produce ~480 inputs; reaching 800 needs the other 4.

## Discrepancy with prior reports

Agent-01 noted: "build_corpus.py — needs internet (HuggingFace datasets, BEIR, Wikipedia, FLORES). Will fail fast on offline boxes; document but skip."

Agent-02 said: "Did not verify offline corpus strata (S1 Shakespeare); deferred to sherlock."

I confirm:
1. Offline-friendly strata DO work; the implementation is correct.
2. The orchestrator monolithic phase=sources cannot run partial offline. There is no `--strata` filter argument. To proceed offline the user would have to comment out S5/S6/S8 in STRATA list OR run only specific collectors via Python import (as I did).
3. S7 temporal makes EXACTLY 120 candidates — agent-01's failure note "S7 built from git log instead of conversation/news/version timelines" confirmed; it's a spec-vs-impl discrepancy not a runtime fault.

## What the simplifier should consider

Add a `--strata` filter argument to `build_corpus.py` to allow offline iteration on S1/S2/S3/S7 only. This would be a small ergonomics fix; not a correctness fix. The full 800-input corpus still needs the other 4 strata.

## Confidence: HIGH (each collector tested independently)
