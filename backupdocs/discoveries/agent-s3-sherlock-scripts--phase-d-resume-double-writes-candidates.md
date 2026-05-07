---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:16:00Z
status: active
severity: high
tags: phase-d, resumability, atomic-writer, double-write, data-corruption
---

# AtomicJsonlWriter opens .partial in append mode then renames — counts existing, but rename overwrites the final

## Evidence
`scripts/shakespeare_sft_augmenter.py:2336-2351`
```python
class AtomicJsonlWriter:
    """Write JSONL to <path>.partial, rename on close. Append-safe for resume."""

    def __init__(self, final_path: Path):
        self._final = final_path
        self._partial = final_path.with_suffix(final_path.suffix + ".partial")
        self._final.parent.mkdir(parents=True, exist_ok=True)
        self._fp = open(self._partial, "a", encoding="utf-8")

    def write(self, obj: dict) -> None:
        self._fp.write(json.dumps(obj, ensure_ascii=False) + "\n")
        self._fp.flush()

    def close_and_finalize(self) -> None:
        self._fp.close()
        os.replace(self._partial, self._final)
```

Called from `generate_sft_candidates` (line 2393) and
`score_and_filter_sft` (line 2589) — both use AtomicJsonlWriter on
DIFFERENT paths (candidates.jsonl vs final.jsonl). So rename doesn't
collide here. BUT:

## The Real Bug: count_existing_by_category counts .partial, not .final

`scripts/shakespeare_sft_augmenter.py:2388-2391`:
```python
partial = out_path.with_suffix(out_path.suffix + ".partial")
existing = count_existing_by_category(partial)
total_existing = sum(existing.values())
log(f"[SFT cand] resuming with {total_existing} existing candidates: {dict(existing)}")
```

Resume logic reads `.partial`. But once `close_and_finalize()` runs, the
`.partial` is renamed AWAY. So:

1. Run A: writes 200 to `.partial`, crashes before finalize.
2. Run B: sees 200 in `.partial`, resumes → writes more → finalizes → `final` gets rename.
3. Run C: BOTH `.partial` is gone (renamed in Run B) AND `final` exists. `count_existing_by_category(partial)` returns 0. Run C REGENERATES EVERYTHING from scratch, overwriting `final` on its own `close_and_finalize()`.

## Second Bug: SIGINT during write corrupts with a half-JSON line

`writer.write()` does `self._fp.write(json.dumps(obj) + "\n"); flush()`.
If SIGINT arrives between write and flush (or mid-write), the `.partial`
file ends with a half-line. Next run's `count_existing_by_category` parses
line-by-line with `json.JSONDecodeError: continue` — so the half-line is
silently skipped, but the file continues to grow. Resume count is short
by 1.

## Third Bug: No resumability across Phase boundaries
If Phase 2 (generation) completes and Phase 3 (scoring) crashes mid-way,
the scoring final file's `.partial` is renamed away → cannot resume
scoring. Have to regenerate candidates too OR edit code to bypass
Phase 2.

## Fix Recommendation

### A. Preserve .partial until final is known good:
```python
def close_and_finalize(self) -> None:
    self._fp.close()
    # Copy, then atomic swap via link-rename. If the rename succeeds,
    # unlink partial AFTER verifying final exists.
    if self._partial.exists():
        shutil.copyfile(self._partial, self._final.with_suffix(".tmp"))
        os.replace(self._final.with_suffix(".tmp"), self._final)
        # Only now drop the partial:
        self._partial.unlink()
```

### B. Check both partial AND final for resume:
```python
def count_existing_by_category(partial: Path, final: Path):
    counts = Counter()
    for path in (final, partial):
        if not path.exists():
            continue
        with open(path) as f:
            for line in f:
                try: rec = json.loads(line)
                except: continue
                cat = (rec.get("_meta") or {}).get("category")
                if cat: counts[cat] += 1
    return counts
```

### C. Write JSONL line atomically by buffering the full line and writing once:
```python
def write(self, obj: dict) -> None:
    line = json.dumps(obj, ensure_ascii=False) + "\n"
    # POSIX guarantees writes <= PIPE_BUF (4KB) are atomic. For larger,
    # os.fsync after each write to ensure flush before SIGINT arrives.
    self._fp.write(line)
    self._fp.flush()
    os.fsync(self._fp.fileno())
```

`os.fsync` is expensive but Phase D generates at ~2-5s per candidate, so
an extra 5-20ms fsync is noise.

## Related
- `./memory/discoveries/agent-s3-sherlock-scripts--no-signal-handlers-anywhere.md`
- Tags: resumability, atomic-write, phase-d
