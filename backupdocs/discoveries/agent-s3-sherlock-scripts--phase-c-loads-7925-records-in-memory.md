---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:05:00Z
status: active
severity: high
tags: memory, streaming, phase-c, dpo-builder, unbounded-list
---

# Phase C loads ALL 7,925 SFT records into a list in RAM before streaming

## Evidence
`scripts/shakespeare_build_dpo_style_pairs.py:383-414`
```python
def load_sft_records(path: Path, limit: int | None) -> list[dict]:
    records: list[dict] = []
    with path.open() as f:
        for i, line in enumerate(f):
            ...
            records.append({...})
    return records
```

And at line 662:
```python
sft_records = load_sft_records(sft_path, args.limit_sft)
```

`shakespeare_sft_final_clean.jsonl` is ~35MB on disk, 7,925 lines. Each record
carries `system`, `prompt`, `chosen` (up to ~1KB), plus copied-in `_meta`.
In Python 3 dict overhead (~300 bytes per key) and string interning costs
often push in-memory JSONL parsing to **5-8x the on-disk size** → ~175-280MB
resident just for this list.

## Why It Matters
Combined with:
1. `tags_idx` dict also held in full memory (7,925 records × more fields, ~80-150MB)
2. The daemon process alongside needs ~20GB
3. `rejected_log_f` is append-only, small

We're looking at ~500MB resident for just Phase C's bookkeeping — not
catastrophic alone, but on a 120GB RAM box already pushed to OOM by
voiceagent processes (see `blockers/agent-04-coder-analyzer--oom-kills-daemon.md`),
every 500MB counts.

## Fix Recommendation
Stream the JSONL instead of materializing:

```python
def iter_sft_records(path: Path, limit: int | None) -> Iterator[dict]:
    """Generator that yields one record at a time. Never holds more than one."""
    with path.open() as f:
        produced = 0
        for i, line in enumerate(f):
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            msgs = obj.get("messages") or []
            if len(msgs) < 3 or msgs[0].get("role") != "system" \
               or msgs[1].get("role") != "user" or msgs[-1].get("role") != "assistant":
                continue
            user = msgs[1].get("content") or ""
            chosen = msgs[-1].get("content") or ""
            if not user.strip() or not chosen.strip():
                continue
            yield {
                "sft_index": i,
                "system": msgs[0].get("content") or "",
                "prompt": user,
                "chosen": chosen,
                "_meta": obj.get("_meta") or {},
            }
            produced += 1
            if limit is not None and produced >= limit:
                return
```

Then in main: `for rec in iter_sft_records(sft_path, args.limit_sft):`.

Same fix for `load_tag_index` (lines 417-440): build `dict[str, dict]`
keyed by SHA-1 but only of the subset of tags actually touched — OR keep
it as-is but **filter to minimal fields** (register, meter.form,
semantic_topic; drop the full `text` and `style_profile_vector`). A tag
record is currently ~2KB each on disk → 16MB of dict hit. Keeping only
three fields: ~200 bytes each → 1.6MB.

## Related
- `./memory/blockers/agent-04-coder-analyzer--oom-kills-daemon.md` (host RAM pressure)
- Tags: memory, phase-c, streaming
