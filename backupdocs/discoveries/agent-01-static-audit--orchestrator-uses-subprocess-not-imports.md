---
namespace: discoveries
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:36:00Z
status: active
tags: exp2, run-exp2, subprocess, orchestration, design
---

# Discovery: run_exp2.py spawns each phase as a subprocess instead of importing

## What
`run_exp2.py:39-43`:
```python
def run(cmd: list[str], config: str) -> None:
    full = cmd + ["--config", config] if "--config" not in cmd else cmd
    res = subprocess.run(full, cwd=repo_path("."), text=True)
    if res.returncode != 0:
        raise Exp2Error(f"Command failed ({res.returncode}): {' '.join(full)}")
```

Each phase runs as a fresh `python3 scripts/exp2/<phase>.py`. So:
- Each phase has its own Python process, importing torch fresh, loading PyTorch CUDA context, etc.
- Phase output is written to disk; next phase reads it back. Hard isolation.
- The orchestrator can't accumulate state across phases (e.g., a single AuditChain instance).

## Pros
- Each phase is independently runnable for debugging (e.g., `python3 scripts/exp2/effective_rank.py`).
- Failure in one phase doesn't corrupt other phases' Python state.
- Can run on different machines if needed.

## Cons
- No shared state means audit-chain wiring must be done per-phase via disk reads. The current spec calls for one chain across the run; subprocess design means each phase appends to its own chain file then merges. (See: agent-01-static-audit--no-audit-chain-built-anywhere.md)
- ~10 seconds startup overhead per phase (torch import).
- 91 pair iterations happen all within a single `run_estimators.py` process, so per-pair startup is fast — but it means `set_global_seed(seed)` mutates global state per pair within that process (see infonce-set-global-seed defect).

## Compliance impact
For the audit-chain blocker: each phase script must be modified to either:
1. Open the master chain file, append events, write back. (race-prone if phases run in parallel; current orchestrator is sequential, so this works.)
2. Each phase writes its own chain file (e.g., `audit_chain_<phase>.json`), and the final phase merges them. (Cleaner.)

Already partially in place: `verify_environment.py` writes `audit_chain_environment.json`. So path 2 is the intended design — needs follow-through.

## Recommended fix
- Each phase: write `audit_chain_<phase>.json`.
- New `audit_chain_finalize.py` script: read all `audit_chain_*.json` files, merge, write `audit_chain.json` with chain_sha256.
- Add this as the last phase in `run_exp2.py PHASE_COMMANDS`.

## Severity
- This is an architectural observation; not a defect by itself.
- Combined with the no-audit-chain blocker, it scopes the fix.
