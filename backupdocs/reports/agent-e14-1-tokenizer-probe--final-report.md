---
namespace: reports
agent_id: agent-e14-1-tokenizer-probe
timestamp: 2026-04-22T15:42:00Z
status: final
tags: e14, task-1, final-report, tokenizer
---

# Final Report — agent-e14-1-tokenizer-probe

## Task completion

**Status: COMPLETE — PASS**

Task 1 from `docs/embedadd/12_task_breakdown_pilot.md` (Verify XLM-RoBERTa
tokenizer support) is done. Both acceptance criteria met:

- [x] Tokenization succeeds without panic
- [x] Token count is 8-15 for "Shall I compare..." (got 15, within bracket)
- [x] Runs cleanly via `cargo test`
- [x] Test IDs are non-negative u32 within vocab range
- [x] encode/decode round-trip preserves content words
- [x] Special tokens `<s>`=0, `</s>`=2 correctly appended

## Files created in repo

| Path | Lines | Purpose |
|---|---|---|
| `/home/cabdru/contextgraph/crates/context-graph-embeddings/tests/bge_m3_tokenizer_probe.rs` | 267 | Probe test with 2 `#[test]` functions |

## Files modified in repo

None.

## Test run output (verbatim)

```
   Compiling context-graph-embeddings v0.1.0 (/home/cabdru/contextgraph/crates/context-graph-embeddings)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 28.63s
     Running tests/bge_m3_tokenizer_probe.rs (target/debug/deps/bge_m3_tokenizer_probe-7060f9a4fe678ec0)

running 2 tests
bge-m3 tokenizer.json path: /home/cabdru/.cache/huggingface/hub/models--BAAI--bge-m3/snapshots/5617a9f61b028005a4858fdac845db406aefb181/tokenizer.json
bge-m3 tokenizer.json size: 17098108 bytes
test resolve_path_finds_existing_snapshot ... ok
Loaded bge-m3 tokenizer: vocab_size=250002
[shakespeare_1] "Shall I compare thee to a summer's day?" -> 15 tokens
  ids:    [0, 159, 29907, 87, 69101, 70, 13, 47, 10, 51065, 25, 7, 5155, 32, 2]
  tokens: ["<s>", "▁S", "hall", "▁I", "▁compare", "▁the", "e", "▁to", "▁a", "▁summer", "'", "s", "▁day", "?", "</s>"]
[shakespeare_2] "Thou art more lovely and more temperate:" -> 12 tokens
[modern_1] "A weather forecast: partly cloudy, 72F." -> 14 tokens
[shakespeare_1] decode: "Shall I compare thee to a summer's day?"
[shakespeare_2] decode: "Thou art more lovely and more temperate:"
[modern_1] decode: "A weather forecast: partly cloudy, 72F."

=== BGE-M3 TOKENIZER PROBE: PASS ===
tokenizers v0.22 successfully loads XLM-RoBERTa SentencePiece
from BAAI/bge-m3 and tokenizes real Shakespeare text.
E14 Phase 1 Task 1: UNBLOCKED for agent-e14-2 (enum plumbing).
test bge_m3_xlm_roberta_sentencepiece_loads_and_tokenizes ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.79s
```

## Memory files written

All absolute paths:

1. `/home/cabdru/contextgraph/memory/discoveries/agent-e14-1-tokenizer-probe--bge-m3-already-cached.md`
   — bge-m3 is fully in HF cache; paths + file inventory for Agents 3/10

2. `/home/cabdru/contextgraph/memory/decisions/agent-e14-1-tokenizer-probe--no-hf-hub-crate.md`
   — rationale for NOT adding `hf-hub` crate; follow existing "local model_path" pattern

3. `/home/cabdru/contextgraph/memory/journal/agent-e14-1-tokenizer-probe--success-tokenizer-works.md`
   — full success log with verbatim test output

4. `/home/cabdru/contextgraph/memory/handoff/agent-e14-1-to-e14-2--tokenizer-verified.md`
   — detailed handoff to Agent 2 including ModelId field-value table, code
   snippet for Agent 3, and gotcha about tokenizer+weights being in separate
   snapshots

5. `/home/cabdru/contextgraph/memory/reports/agent-e14-1-tokenizer-probe--final-report.md`
   — this file

## Blockers found

**None.** No `memory/blockers/` file written because nothing blocks the pipeline.

## Key discoveries (summary)

1. **bge-m3 already cached locally.** No download needed.
   `~/.cache/huggingface/hub/models--BAAI--bge-m3/snapshots/5617a9f61b028005a4858fdac845db406aefb181/tokenizer.json`
2. **Tokenizer loads cleanly with existing v0.22 crate.** No patch, no new dep.
3. **Tokenizer and safetensors weights are in DIFFERENT HF snapshots.** Agent
   3 must handle this (see handoff for three options).
4. **vocab_size = 250002**, special tokens `<s>`=0/`<pad>`=1/`</s>`=2/`<unk>`=3 —
   standard XLM-RoBERTa, as expected.
5. **15 tokens for "Shall I compare thee to a summer's day?"** — inside the
   8-15 acceptance bracket.
6. **`tokenizer.json` ships with `"truncation": null`.** Agent 3's loader
   must configure `with_truncation(max_length=8192)` explicitly (the other
   embedders do this too; standard pattern).

## Decisions

1. **No `hf-hub` crate.** Follow existing local-path pattern (Decision doc
   in memory/decisions/). Avoids ~50+ HTTP-stack deps.
2. **Probe uses `assert!` not `Result<(), _>`.** Matches existing test
   style in `qodo_embed_integration.rs`. Fails loudly with descriptive
   messages.
3. **Two separate `#[test]` functions.** One for the main probe, one for the
   path resolver. Keeps failure localization sharp if the resolver breaks
   later (e.g. after an HF cache layout change).

## Confidence

**HIGH.**

Rationale:
- Real test against real HF artifact (not a stub)
- 6 independent assertion categories all pass (path, vocab, count, id-range,
  round-trip, special-tokens)
- Existing codebase has 9 loaders using the identical `Tokenizer::from_file`
  pattern, all in production
- Output is deterministic and reproducible (`cargo test` ran twice, same result)

## What I need from coordinator next

Nothing. Agent 2 is cleared to start immediately using
`memory/handoff/agent-e14-1-to-e14-2--tokenizer-verified.md` as their brief.

## Notes / loose ends

- The probe is kept in `tests/` so it runs on `cargo test --workspace`
  going forward. Agent 9 (simplifier) may want to fold this into a richer
  `bge_m3_dense_integration_test.rs` once the full pipeline exists — but
  it's self-contained and adds no extra deps, so leaving it standalone is
  also defensible.
- If CI ever runs on a machine without bge-m3 pre-cached, the test will
  fail with a clear `huggingface-cli download BAAI/bge-m3 tokenizer.json`
  instruction in the panic message. This is by design — no silent skips.
