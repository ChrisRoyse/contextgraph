---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T22:57:00Z
status: active
severity: low
tags: exp2, common, fail-loud, helpers, success
---

# Success: scripts/exp2/common.py is consistently fail-loud

## What's good
- `Exp2Error` raised on every dependency-missing or invalid-input case (lines 70-71 class, then 86, 90, 129, 174, 179, 182).
- `set_global_seed` (lines 161-174) wraps the import-torch path in `try/except` — but the except **re-raises as Exp2Error**, satisfying C2 (fail loud).
- `l2_normalize` (lines 177-184) detects non-finite values and zero-norm rows up front, with the offending indices in the message.
- `canonical_json` uses `sort_keys=True, separators=(",", ":")` for a stable sha256.
- `write_json` uses tmp-then-replace (atomic write); good for FSV reproducibility.
- `EMBEDDER_KEYS` is the canonical 14-slot panel matching `04_data_corpus.md` and `05_technical_spec.md` §5b.
- `EXPECTED_DIMS` matches the per-embedder dim table in `03_methodology.md` §3 (with notes: E6/E12/E13/E9 are post-projection sizes, which is honest).

## Issue: setup_logging never closes the FileHandler
Minor leak: `setup_logging` (line 134) attaches a `FileHandler` per call but never has a `close()` companion. Multiple invocations from the same process accumulate handlers (the `logger.handlers.clear()` at line 137 mitigates this for repeated calls of the same logger name). Acceptable for a one-shot harness.

## Issue: token_count uses whitespace split
`token_count(text)` (line 193) returns `len(text.split())` — that's word-split, not real tokenizer tokens. The spec (`04_data_corpus.md` §4) requires `50 ≤ tokens ≤ 800` per Whisper / E1 limits. A whitespace tokenizer can disagree with the embedder's BPE tokenizer by 1.3-2x. Document or use the real tokenizer. Flag as low; agent-02 should at minimum log a warning.
