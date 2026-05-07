---
namespace: reports
agent_id: agent-10-simplifier-shakespeare
timestamp: 2026-04-14T19:50:00Z
status: active
tags: report, final, agent-10, shakespeare, code-review, simplification
---

# Final Report: agent-10-simplifier-shakespeare

## Task Completed: yes

Reviewed all four Shakespeare-pipeline scripts (chunker, ingest, extract,
queries, summary — five files total) for correctness, simplicity,
reusability, and error handling. Did not edit any file. Filed eight
discovery files; this report.

## Summary

The pipeline works end-to-end on the current corpus (1,189 ingested, 0 errors,
8/10 top-10 queries) but has several silent breakage points that will hurt
either (a) the next operator running with default flags, or (b) the next
corpus that isn't a Project-Gutenberg-style file with a TOC.

The two HIGH-severity issues are both contract mismatches between scripts:
the ingest script's default output filename does not match what extract,
queries, and summary read; and the demo summary expects `verdict` /
`top_rationale` fields the queries script never writes — meaning the final
demo doc shows "ERR" for every known-answer proof.

The chunker's scene regex is uppercase-only while 32.6% of scene headers in
the actual corpus are mixed-case (`Scene I.` rather than `SCENE I.`),
silently mis-attributing 365 scene boundaries. On a future non-Gutenberg
edition, this regex (plus the TOC-occurrence heuristic in
`_collect_title_bounds`) will likely fail to find any plays at all.

The remaining MEDIUM/LOW findings cover an O(N²) dict-init + KeyError risk
in queries, three-way duplication of the JSON-RPC client across scripts, an
overly narrow `except ImportError` in the parquet section, a
`{coherence:.4f}` format that crashes on null, and three magic-number
importances in the ingest script.

## Files Created or Modified in the Repo

None. Per task instructions, the agent did not edit source files.

## Files Written to ./memory/

- `discoveries/agent-10-simplifier-shakespeare--ingest-log-filename-mismatch.md` — HIGH: ingest defaults to `_log.jsonl`, consumers read `.jsonl`
- `discoveries/agent-10-simplifier-shakespeare--demo-summary-verdict-field-missing.md` — HIGH: summary reads `verdict`/`top_rationale` that queries never writes
- `discoveries/agent-10-simplifier-shakespeare--scene-regex-uppercase-only.md` — HIGH: 365 of 1,120 scene markers (32.6%) silently lost on actual corpus
- `discoveries/agent-10-simplifier-shakespeare--title-bounds-fragile.md` — MEDIUM: requires TOC + body double occurrence; non-Gutenberg corpora break
- `discoveries/agent-10-simplifier-shakespeare--queries-quadratic-init-and-key-error-risk.md` — MEDIUM: O(N²) `min(...)` per iteration + dict-order dependency
- `discoveries/agent-10-simplifier-shakespeare--rpc-helpers-duplicated-three-times.md` — MEDIUM: same JSON-RPC client copy-pasted 3x with subtle differences
- `discoveries/agent-10-simplifier-shakespeare--summary-parquet-block-narrow-except.md` — MEDIUM: catches only ImportError; runtime errors crash the demo
- `discoveries/agent-10-simplifier-shakespeare--coherence-none-crashes-format.md` — MEDIUM: `{c['coherence']:.4f}` crashes on null
- `discoveries/agent-10-simplifier-shakespeare--ingest-importance-and-rationale-magic-numbers.md` — LOW: hard-coded 0.70/0.75/0.80 with contradictory docstring
- `reports/agent-10-simplifier-shakespeare--final-report.md` — this file

## Decisions Made

- Did NOT edit any of the five Python scripts. Per the user's hard rule
  ("DO NOT edit the files. Flag only.") all findings were filed as
  discoveries.
- Severity classification: contract mismatches that produce silently wrong
  demo output → HIGH; quietly-wrong scene attribution on the live corpus → HIGH;
  fragile portability + duplication that bites the next person → MEDIUM;
  cosmetic + doc-vs-code drift → LOW.
- Skipped pure stylistic nits (e.g., one-letter `s` socket variable, missing
  type hints on a few helpers) per the task's "Skip nits unless the code rots
  in <1 week" rule.

## Discoveries

(All eight discovery files cited above. Top three for handoff:)

- **The ingest log filename default is wrong.** Pipeline runs with all defaults
  produce a demo doc that says "0 ingested" while RocksDB is full.
  Impact: every operator who runs the README block in `shakespeare_demo_summary.py`.

- **Demo summary's verdict table will always show ERR.** Field schema drift
  between `shakespeare_demo_queries.py` (writes `best`/`strategies`) and
  `shakespeare_demo_summary.py` (reads `verdict`/`top_rationale`).
  Impact: the single most important visual proof in the GTM demo is invisible.

- **Scene regex drops 32% of scenes on real input.** UPPERCASE-only
  `^SCENE\s+...` misses 365 of 1,120 scene markers.
  Impact: act/scene metadata is wrong for a third of all stored scenes;
  any future per-act analysis is poisoned. On non-Gutenberg editions, likely
  produces zero scenes.

## Failures

None — all reviews completed. No build/test runs were attempted (per task
scope: review only, no edits, no builds).

## Successes

- Verified every claim in the user's prompt against actual files: the
  chunker outputs do parse cleanly, the ingest payload shape matches what
  the daemon's `store_memory` expects, the four retrieval strategies in
  queries are valid MCP tools, and the parquet block IS gated on import.
- Cross-checked corpus assumptions against `complete_works.txt` directly
  (line counts, title occurrences, scene/act marker case distribution) —
  found three correctness issues only by counting.

## Blockers (if any)

None.

## Tests / Build

- Tests added: none (review-only task).
- Tests passing: not run.
- Build status: not run.

## For the Next Agent

- Start by reading: `./memory/discoveries/agent-10-simplifier-shakespeare--ingest-log-filename-mismatch.md`
  and `./memory/discoveries/agent-10-simplifier-shakespeare--demo-summary-verdict-field-missing.md` —
  these are 5-minute fixes that immediately make the demo doc correct.
- Watch out for: the three HIGH findings interact. Fixing the verdict-field
  discrepancy first makes it obvious whether queries actually pass; fixing
  the scene regex changes the chunk count from 1,189 to ~1,550, which
  invalidates the cached `/tmp/shakespeare_chunks.jsonl` and forces a re-ingest
  (~72 min) before the demo numbers are honest.
- Priority order for hardening:
  1. Filename default in ingest (1 line)
  2. Verdict/top_rationale derivation in summary (5 lines)
  3. Scene regex case-insensitive + min-count assertion in chunker (2 lines + 3 lines)
  4. Extract `_mcp_client.py` helper and rewire all 3 scripts (one-time refactor, ~50 lines net deletion)
  5. Defensive None-handling in summary (parquet + coherence)

## Confidence: high

All eight findings were verified directly against the source files
(`/home/cabdru/contextgraph/scripts/shakespeare_*.py`) and the corpus
(`/home/cabdru/contextgraph/shakespear/complete_works.txt`).
The two HIGH contract mismatches and the scene-regex case mismatch are
trivially reproducible. None of the findings depend on running the daemon.
