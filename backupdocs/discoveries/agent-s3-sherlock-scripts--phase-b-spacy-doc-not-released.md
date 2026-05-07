---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:34:00Z
status: active
severity: low
tags: phase-b, spacy, memory-per-record
---

# Phase B: spaCy Doc objects are ~2-5x the text size each — 7,925 docs freed only by ref

## Evidence
`scripts/shakespeare_stylistic_analyzer.py:1062-1095` (analyze_record):
```python
def analyze_record(memory_id: str, text: str, source: str,
                   daemon: DaemonClient) -> dict:
    doc = _NLP(text)   # spaCy Doc
    ...
    devices    = _analyze_devices(text, doc)
    ...
    sentence   = _analyze_sentence_shape(text, doc)
    ...
    profile    = daemon.score_passage(text)  # may raise
    return {...}
```

Per-record behavior:
- `doc = _NLP(text)` allocates a spaCy Doc (token spans, dep tree, vectors)
- When `analyze_record` returns, `doc` falls out of scope, gets garbage
  collected on next allocator cycle
- spaCy's tokenizer caches token vocab entries for reuse — these
  accumulate permanently

## Non-Issue (Checked)
- `_NLP` is loaded ONCE at module import (line 76), not per-record. Good.
- `pronouncing` library is lightweight (dict-based), no per-record allocation.
- The archaic marker set and topic keyword dicts are module-level constants.

## Minor Issue
- After processing 7,925 records, spaCy's vocab hash table will contain
  every unique token ever seen → ~20-50MB permanent growth. This is NORMAL
  spaCy behavior, not a leak. Mitigatable by re-creating the pipeline
  every N records but the cost isn't worth it.

## No Fix Needed
The analyzer's memory handling is acceptable. The crash wasn't caused by
the analyzer — it was caused by voiceagent processes (see
`blockers/agent-04-coder-analyzer--oom-kills-daemon.md`). This finding
is informational.

## Related
- Tags: phase-b, spacy, informational
