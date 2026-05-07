---
namespace: discoveries
agent_id: agent-s3-sherlock-scripts
timestamp: 2026-04-17T18:22:00Z
status: active
severity: medium
tags: phase-c, sys-exit, checkpoint, state-loss
---

# Phase C: _maybe_abort_on_budget raises SystemExit(1) and skips state save

## Evidence
`scripts/shakespeare_build_dpo_style_pairs.py:842-853`
```python
def _maybe_abort_on_budget(scorer: DaemonStyleScorer) -> None:
    """Abort (raise SystemExit 1) if daemon failure rate exceeds budget
    AND we've made at least 20 calls (avoid aborting on a single startup flap)."""
    if scorer.total_calls < 20:
        return
    if scorer.failure_rate > DAEMON_FAIL_BUDGET:
        print(f"[fatal] daemon failure rate {scorer.failure_rate:.4%} ...",
              file=sys.stderr)
        raise SystemExit(1)
```

Called at lines 709, 740 INSIDE the main loop. The main `try:` block at
line 684 has `finally:` at line 798 that closes rpc, partial_f,
rejected_log_f. So the writers ARE flushed. Good.

BUT: `SystemExit(1)` bypasses the final state checkpoint block at
lines 813-819 that runs AFTER the `finally`. So state.json is
`processed_sft_indices` as of the LAST 100-record checkpoint, not the
true latest. Resume will re-process up to 99 records.

Also: the completion marker + atomic rename at 822-824 is skipped, so
`output_is_complete()` returns False on next run → re-run will
re-process partial → possibly double-write variants for the same
sft_index if the processed_sft_indices was not saved yet.

## Fix Recommendation
Move state save into a `try/finally` that catches SystemExit:

```python
try:
    for rec in iter_sft_records(...):
        ...
except SystemExit:
    pass  # let finally run
finally:
    state.update({
        "processed_sft_indices": sorted(processed),
        "pairs_written": pairs_written,
        "rejected_pairs": rejected_pairs,
        "last_checkpoint_ts": time.time(),
    })
    save_state(state_path, state)
    try: rpc.close()
    except: pass
    try: partial_f.close()
    except: pass
    try: rejected_log_f.close()
    except: pass
```

And instead of `raise SystemExit(1)` from `_maybe_abort_on_budget`,
set a module-level `_ABORT_REQUESTED = True` flag and check it at the
top of the outer for loop. Cleaner control flow, state always saved.

## Related
- Tags: phase-c, state-loss, abort
