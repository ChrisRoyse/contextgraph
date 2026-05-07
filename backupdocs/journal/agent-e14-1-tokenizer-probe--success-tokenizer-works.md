---
namespace: journal
agent_id: agent-e14-1-tokenizer-probe
timestamp: 2026-04-22T15:35:00Z
status: success
tags: e14, task-1, tokenizer, probe, bge-m3
---

# SUCCESS: BGE-M3 XLM-RoBERTa SentencePiece tokenizer probe passes

## Task

Task 1 from `docs/embedadd/12_task_breakdown_pilot.md`: verify the existing
`tokenizers` v0.22 crate can load `BAAI/bge-m3/tokenizer.json` and tokenize
real Shakespeare text.

## Result

**PASS.** E14 is unblocked on the tokenizer front. Agent 2 can proceed
immediately.

## Evidence (verbatim test output)

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

## Assertions that passed

- `Tokenizer::from_file` loads 17,098,108-byte tokenizer.json without error
- `vocab_size=250002` (confirms XLM-RoBERTa-Large)
- "Shall I compare..." → 15 tokens (inside 8-15 acceptance window)
- "Thou art more lovely..." → 12 tokens (inside [5,50])
- "A weather forecast..." → 14 tokens (inside [5,50])
- All token IDs in [0, vocab_size)
- `<s>`=0 at start of every encoding
- `</s>`=2 at end of every encoding
- decode(encode(x)) round-trip preserves all content words (tested with a
  4+ character pick per input)

## Files created

- `/home/cabdru/contextgraph/crates/context-graph-embeddings/tests/bge_m3_tokenizer_probe.rs`
  (267 lines, 2 `#[test]` functions)

## Files modified

None. Zero source changes; probe is an additive test.

## Environment

- Cargo: `cargo test --workspace -p context-graph-embeddings --test bge_m3_tokenizer_probe -- --nocapture`
- Features: default (`candle` is on by default, which enables `tokenizers` v0.22)
- Build time: 28.63s (first-time candle + cudarc compile, subsequent runs ~3s)
- Test time: 3.79s
- Workspace state: clean, no source modifications

## Key discovery

bge-m3 already fully in HF cache at
`~/.cache/huggingface/hub/models--BAAI--bge-m3/snapshots/5617a9f61b028005a4858fdac845db406aefb181/`.
No `hf-hub` crate needed; no network download needed. See
`discoveries/agent-e14-1-tokenizer-probe--bge-m3-already-cached.md` for
full file inventory.
