---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T22:59:00Z
status: active
severity: medium
tags: exp2, corpus, S8, multilingual, build-corpus, spec-mismatch
---

# Defect: build_corpus.collect_multilingual joins consecutive sentences into windows; result is not 50 English+5×10 translations

## Where
`scripts/exp2/build_corpus.py:218-239` — `collect_multilingual`

## Spec
`docs/exp2/04_data_corpus.md` §S8:
> Selection: 50 English originals + their translations to 5 languages (Spanish, French, German, Chinese, Arabic) — 100 sentences total.

A correct implementation pairs 50 English sentence_ids with their parallel translations in the other 5 languages, totaling exactly 100 ITEMS (with 50 English originals + 10 each of 5 translations OR 50 originals + 50 of one combined translated set; spec's exact wording is 50 English + translations). Either way: parallel structure is essential to "stress E14 multilingual."

## Observed
The implementation:
- Iterates 6 language codes (English plus 5 targets).
- For each language, builds **moving 8-sentence windows** of consecutive FLORES dev sentences.
- Stops at 25 windows per non-English language.
- Returns `out[:120]` (MIN_CANDIDATES).

Issues:
1. The 8-sentence window is **not** parallel across languages. FLORES `dev` does have parallel `id` alignment, but the implementation reads each language stream independently with no `id` join.
2. There's no operator-spec'd "50 English + 50 translations" structure; it's just "windows".
3. `len([r for r in out if r["metadata"]["language"] == name]) >= 25` produces 6 × 25 = 150 candidates, which is more than the spec's 100 — fine for MIN_CANDIDATES=120 headroom, but the spec balance is lost.
4. The English language is included in `langs` and contributes to the total — fine, but no "50 English originals" anchor exists.

## Why it matters
E14 (BGE-M3 multilingual) is supposed to be stressed by *parallel* sentences (the same content in multiple languages). MI between E14 and E1 (English semantic) becomes high when the same content appears in both. Without parallelism, the multilingual embedder is just being fed random text in different languages, and MI estimates against E1 will reflect content-overlap, not multilingual-axis usage.

## Recommended fix
1. Use FLORES `id` field to join parallel sentences across languages (each FLORES row across language streams has same `id` for the same source sentence).
2. Sample 50 sentence IDs from English; emit one record per (lang, id) pair = 6×50 = 300 candidates, then sample 100.
3. Ensure metadata records the parallel id so downstream replays can verify.

## Cross-check
- Agent-02 should sanity-check per-stratum balance after a corpus build attempt.
- This may be unrunnable without internet access to FLORES dataset; flag as a blocker if it can't be tested live.
