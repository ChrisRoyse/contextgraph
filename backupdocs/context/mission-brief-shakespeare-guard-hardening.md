---
namespace: context
agent_id: coordinator
timestamp: 2026-04-17T13:00:00Z
status: active
tags: mission, shakespeare, 3-layer-guard, fail-fast, no-fallback
---

# Mission: Shakespeare 3-Layer Guard Hardening (Fail-Fast)

## Objective
Make the three-layer Shakespeare style guard **always operational** when the chat dashboard is online. No workarounds, no fallbacks, no silent failures. If any layer is broken, the system must error out with actionable logs.

## The Three Layers (source of truth = `scripts/shakespeare_chat_dashboard.py`)
1. **LoRA (learned)** — fine-tuned Gemma 3 12B, runs in `shakespeare_inference_server.py` on `127.0.0.1:8766`
2. **Constrained decoder (deterministic)** — logit processor in the inference server boosts archaic / suppresses modern + foreign tokens
3. **13-embedder guard (verification)** — `DaemonScorer` in `shakespeare_chat_dashboard.py` stores generated text in Context Graph daemon on `127.0.0.1:3100`, searches it, and scores per-embedder agreement against precomputed `STYLE_CENTROID` / `STYLE_P5`

## Root Cause (confirmed 2026-04-17)
- `ss -lntp | grep 3100` → nothing. **Daemon processes alive but NOT listening.** Two PIDs (1053763, 1177380) in `S (sleeping)` state with only `CLOSE_WAIT` client sockets visible in `lsof`. No accept loop.
- `score_embedders` catches the resulting connection-refused exception at `scripts/shakespeare_chat_dashboard.py:299-303`, silently returns `(0, [], ~0.1ms)`.
- `full_score` line 322-323 contains a **vocab-density bypass**: if `vocab_density >= 0.05`, `overall_pass = True` regardless of embedder outcome. Any archaic-ish text passes.
- `respond` line 408-410 contains a **"BEST" fallback**: after `MAX_ATTEMPTS` all fail, picks highest-scoring attempt and marks it "BEST" instead of FAIL.
- Combined: when guard is offline, vocab bypass yields PASS; when guard runs and all attempts fail, BEST fallback yields output anyway. Prompt injections sail through because no hard gate exists.

## Non-Negotiables
- **NO BACKWARDS COMPATIBILITY.** Remove vocab bypass. Remove BEST fallback. Break callers if needed.
- **NO WORKAROUNDS.** Every layer must actually run every time or the request errors out.
- **NO MOCK DATA in tests.** Use real daemon + real inference server.
- **NO TESTS THAT PASS ON BROKEN STATE.** If the system is broken, tests must fail.
- **FAIL FAST** on daemon unreachable, schema mismatch, empty embedder scores, or retry exhaustion.
- **ROBUST ERROR LOGGING** — every failure path must emit enough info to diagnose without re-running.

## Full State Verification Targets (what to inspect post-change)
1. `ss -lntp` shows daemon listening on 3100 after startup.
2. `store_memory` MCP call returns a `fingerprintId`, and a follow-up `get_memory_fingerprint` shows all 13 embedder indices populated in RocksDB.
3. `search_graph` returns per-embedder scores in nested `{semantic, relational, structural, temporal}` buckets with 9+ non-null entries for Shakespeare-vocab input.
4. Dashboard refuses to serve HTTP if `8766` or `3100` is unreachable at startup.
5. Dashboard refuses to return a reply if all attempts fail — HTTP 503 + explicit error, never a "BEST" fallback.
6. Prompt-injection test: modern English output must produce FAIL verdict, trigger retry, and (if retries exhaust) return 503.

## Files Under Change
- `scripts/shakespeare_chat_dashboard.py` — guard rewrite
- `scripts/shakespeare_inference_server.py` — possibly add /health endpoint
- new `scripts/shakespeare_dashboard_smoke_test.py` — real-data happy path + edge case harness

## Do NOT Touch
- `crates/context-graph-*/` — backend is working per prior audits. Only rewrite Python dashboard logic.
- Prior typed-edges-v1 context files in `./memory/context/`.
