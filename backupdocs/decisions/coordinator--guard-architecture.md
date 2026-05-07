---
namespace: decisions
agent_id: coordinator
timestamp: 2026-04-17T15:28:00Z
status: active
tags: architecture, fail-fast, self-heal, guard, dashboard
---

# Guard Architecture Decisions (Locked)

## D1: Fail-fast on ALL three layers
If any layer is broken at request time, the dashboard MUST return HTTP 503 with a structured error. No silent downgrade, no vocab bypass, no "BEST" fallback.

**Why**: Two independent masking paths (vocab bypass at line 322-323 and BEST fallback at line 408-410) turned the 3-layer design into a 1-layer vocab-counter when the daemon died. Prompt injections passed through.

**How to apply**: Remove both fallbacks. Replace with explicit `raise GuardError(...)`. HTTP handler converts to 503.

## D2: Self-heal is a BACKGROUND concern, not a per-request concern
Per-request code fails fast. A background supervisor watchdog heals services. On the NEXT request after a heal completes, the request succeeds normally. A request during an active outage gets 503 — it does NOT wait for heal.

**Why**: Blocking a chat request for 30+ seconds while a 30GB model reloads is worse UX than a clear "service recovering, try again" 503.

**How to apply**:
- Startup probe (blocking): verify 3100 and 8766 both respond OR refuse to bind 8765.
- Watchdog thread: every 15s probe both services. If down, attempt heal. Max 3 heals per 300s (flap protection). Log every heal attempt with timestamp + PID + exit code.
- Per-request: open short-timeout socket. If connect/timeout/schema-error: raise, return 503. Do NOT trigger heal synchronously.

## D3: Heal daemon by delegating to existing `scripts/ensure_daemon.sh`
That script is already correct, idempotent, PID-file-aware. No reinvention.

**Why**: Prior session already validated ensure_daemon.sh. Rewriting daemon-start logic in Python would duplicate behavior and drift.

**How to apply**: Supervisor's `heal_daemon()` invokes `scripts/ensure_daemon.sh` as a subprocess. Check exit code. If non-zero, log stderr and give up for this cycle.

## D4: Inference server is NOT auto-started by dashboard
The inference server holds ~30GB VRAM. Auto-starting it could clash with training jobs. Inference must be started by the user.

**Why**: User explicitly mentioned training pipelines use GPU. Silent auto-start could break a long-running training run.

**How to apply**: Supervisor's `ensure_inference_listening()` only PROBES. If down, logs a loud actionable message (`[supervisor] ERROR: inference server not running — run: python3 scripts/shakespeare_inference_server.py --port 8766`) and raises. No auto-start.

## D5: Schema assertions are MANDATORY in score_embedders
The current code defensively treats missing keys as 0. This masked the daemon-down state.

**Why**: A missing `embedderScores` key is a PROTOCOL error, not a "score 0" outcome. Silent 0 corrupts the centroid-comparison logic.

**How to apply**:
- `assert store["fingerprintId"]` — else `GuardError("store_memory missing fingerprintId")`
- `assert search["results"]` non-empty — else `GuardError("search returned no results for just-stored memory")`
- `top = results[0]; assert top["fingerprintId"] == store["fingerprintId"]` — else `GuardError("top result is not the just-stored memory; HNSW insert race?")`
- `es = top["embedderScores"]; assert all(bucket in es for bucket in ["semantic","relational","structural","temporal"])` — else `GuardError("response missing embedderScores bucket <x>")`
- For each of the 9 STYLE_CENTROID embedders, pull the expected value. If MORE than 2 of the 9 are null → `GuardError("too many null embedder scores; embeddings not fully populated")`

## D6: Guard retries ONLY on FAIL verdict, not on errors
- FAIL (style_score < 0.6 OR vocab_density < 0.02): retry at higher temp, up to MAX_ATTEMPTS=3
- ERROR (GuardError): do NOT retry. Return 503. Watchdog handles heal asynchronously.

**Why**: Retrying an offline daemon just queues up more failures and delays the 503.

## D7: Memory cleanup on error paths
Every `store_memory` MUST be paired with `forget_concept` in a try/finally. Audit trail MUST show the matched STORE+FORGET per chat turn. Orphaned guard-check memories are a bug.

**How to apply**: `try/finally` wrapping in `score_embedders`. FSV verifies via `get_audit_trail`.

## D8: Log format is structured
All supervisor/guard logs prefixed `[supervisor]`, `[guard]`, or `[dashboard]`. Include:
- ISO timestamp
- Attempt index (for retries)
- Per-embedder actual vs threshold (on FAIL)
- Exception class + full message (on ERROR)
- PID of healed daemon (on HEAL)

**Why**: User mandated robust error logging to diagnose failures without re-running.

## Related
- See also: ./memory/context/mission-brief-shakespeare-guard-hardening.md
- See also: ./memory/discoveries/coordinator--search-graph-schema-confirmed.md
