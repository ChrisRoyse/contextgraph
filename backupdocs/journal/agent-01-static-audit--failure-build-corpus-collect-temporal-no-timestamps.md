---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T22:58:00Z
status: active
severity: medium
tags: exp2, corpus, S7, temporal, build-corpus, spec-mismatch
---

# Defect: build_corpus.collect_temporal builds S7 from `git log` instead of conversation/news/version timelines

## Where
`scripts/exp2/build_corpus.py:200-215` — `collect_temporal`

## Spec
`docs/exp2/04_data_corpus.md` §S7:
> Source: chronologically-ordered conversation turns, news article timeline series, scientific paper version histories.
>
> Selection: 100 multi-turn snippets each with timestamps. Mixed sources (10 conversation snippets, 5 news timelines, 5 paper versions = 100 paragraphs).

(Note: the spec arithmetic 10+5+5 = 20 already mismatches "= 100"; agent-04 should flag the spec; but the implementation also does **not** match either reading.)

## Observed
The implementation pulls the **git log of the repo** (`git log --pretty=format:%H%x09%ad%x09%s%n%b`) and joins consecutive commit messages into windows. So:
- `record["text"]` will be lines like `"Timeline event 0: a7ef6df1\t2026-04-...\tdocs(ideas): add Context Graph speculative horizons research notes ..."`.
- These are **commit messages**, not conversation turns / news timelines / paper versions.
- The timestamps are not embedded in the text itself in any structured way — they are just an ISO date pasted next to the SHA.
- This stratum will under-stress E2 (recency)/E3 (periodic)/E4 (positional) because git messages don't have the kind of timestamped serial structure those embedders expect (PE encodes a real recency time delta, not a literal date string).

## Why it matters
- C1 (no fallbacks): substituting commit messages for the spec'd S7 content is a stratum-design fallback the operator did not approve.
- C8 (audit chain): each S7 input's `metadata.source` will be `"git_log"`, not "conversation snippet" / "news timeline" / "paper version". An external reviewer reading the published manifest sees the substitution.
- The MI estimates for E2/E3/E4 vs other embedders may be artificially low, which biases N_eff *upward* (those pairs look independent because the temporal embedders see no temporal signal).

## Recommended fix
Either:
1. Implement S7 per spec — pull session.transcripts + a public news-timeline corpus + a paper-versions source. Author the file deterministically.
2. Update spec `04_data_corpus.md` §S7 to acknowledge that for v1 the temporal stratum is sourced from git-log and that this is a known limitation that will be addressed in v2.

Either path is fine; both must be logged. Current state is silent drift.

## Cross-check
Agent-02 should `python3 scripts/exp2/build_corpus.py --phase sources` (if data avail) and then `head -3 scripts/exp2/strata/S7_temporal_sequence.json` to confirm.
