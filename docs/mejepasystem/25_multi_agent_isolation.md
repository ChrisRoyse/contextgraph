# 25 — Multi-Agent Isolation

Source files covered: `docs/futurebuild/23_MULTI_AGENT_ISOLATION.md`, `crates/context-graph-mejepa-shift-subscriber/`, `crates/context-graph-mcp/src/handlers/tools/reality_loop/shift_log.rs`.

> Multi-tenant + multi-session isolation for concurrent agents sharing the same daemon.

---

## 1. Isolation primitives

| Surface | Isolation key |
|---|---|
| Shift log JSONL | `session_id` (per-session file) |
| `CF_MEJEPA_LIVE_PREDICTIONS` | `session_id` (latest) + `(session_id, prediction_id)` (history) |
| `CF_MEJEPA_SHIFT_WATERMARK` | `session_id` (byte offset per JSONL) |
| `CF_MEJEPA_AGENT_FEEDBACK` | `(prediction_id, agent_id, ts)` |
| EditLock | `session_id` (per-session, in `harness_apply_line_window_edit`) |
| `mejepa_record_agent_feedback` rate limit | `agent_id` (100/hour) |
| Active-learning queue entries | `(prediction_id, agent_id)` tagged |
| BlackBox ledger | `tenant_id` (BlackBoxFix-level) |

---

## 2. Session ID lifecycle

```
[Claude Code session start]
   → SessionStart hook generates UUID-style session_id (`01HXY...`)
   → Persisted in /mnt/d/contextgraph/state/cgreality/active_runtime_root (per active session)

[Each tool call]
   → CLAUDE_SESSION_ID env var passed through

[On hook fire]
   → context-graph-cli hooks <subcommand> --session-id <id>
```

`session_id` is the cross-cutting key for shift log, predictions, EditLock, watermark, and active-learning entries.

---

## 3. Concurrent edits to same file across sessions

Per `docs/futurebuild/22 §6` + `docs/futurebuild/23 §3`:

| Scenario | Detection | Resolution |
|---|---|---|
| Agent A edits `foo.py` at SHA `x` → after_sha `y`; Agent B then reads `foo.py` and sees `y` (current). | Normal flow. | A's prediction lands first; B sees current state via PreToolUse SHA. |
| Agent A and Agent B concurrently start an Edit on `foo.py`. | `harness_apply_line_window_edit` acquires per-session EditLock. | One waits via `CCREALITY_EDIT_LOCK_HELD`. |
| Agent A's session crashes mid-edit, leaving stale lock. | Lock has TTL (default 5 min); auto-released. | Subsequent A or B acquires lock. |
| External-to-system file change (manual edit, IDE save) between sessions. | `reality_shift_compare_to_my_view {session_id, files}` returns `external_drift_detected=true`. | Both sessions must re-perceive before continuing per reality-shift principle. |

---

## 4. Per-agent rate limits

| Tool | Limit | Reason |
|---|---|---|
| `mejepa_record_agent_feedback` | 100/hour/agent_id | anti-DoS; agents shouldn't be feedback-spamming |
| `mejepa_verify` | unbounded; CUDA queue serializes | predictor forward GPU-bound; serialization is at GPU |
| `mejepa_predict_latest` | unbounded | pure RocksDB read |
| `mejepa_explain_prediction` | unbounded | RocksDB read + light rerank |

Rate limit data structure: per-`agent_id` rolling-window counter in `CF_MEJEPA_AGENT_FEEDBACK` index.

---

## 5. Per-session shift subscriber consumption

The shift subscriber is **NOT per-session** — there's one supervised tokio task that tails ALL session JSONLs. Per-session state lives in:

- `CF_MEJEPA_SHIFT_WATERMARK` keyed by `session_id` (byte offset per JSONL)
- `InstrumentCache` keyed by `(slot, file_sha)` — shared across sessions (file SHA collisions imply same content)

On restart: `ShiftSubscriber::open` reconstructs ALL session cursors from CF_MEJEPA_SHIFT_WATERMARK and resumes tailing from the persisted byte offsets.

---

## 6. Per-session predictions

`CF_MEJEPA_LIVE_PREDICTIONS` has dual indexing:

```
key                                            → value
(session_id, latest)                          → RealityPrediction (overwritten on each verify)
(session_id, prediction_id, ts)               → RealityPrediction history
```

`mejepa_predict_latest` reads the `latest` key. `mejepa_inspect_prediction` (futurebuild) reads from history by `prediction_id`.

---

## 7. Operator session

The **operator** doesn't have a Claude Code `session_id`. Operator actions go through:
- MCP tools called directly (via `nc -U /mnt/d/contextgraph/state/locks/mcp.sock` or CLI wrappers)
- CLI commands (`context-graph-cli mejepa ...`)

Operator labels in `state/gold-labels/` are tagged `operator_override=true` but not tied to a session ID.

---

## 8. Cross-session leakage prevention

| Path | Leakage prevention |
|---|---|
| Shift subscriber → observe() | every shift carries `session_id`; observe() does NOT discriminate per session (training data unified) — predictions are unified BUT served per-session |
| Predictions back to agent | strictly per-session via `mejepa_predict_latest(session_id)` |
| Agent feedback | bound to `(prediction_id, agent_id)`; agent A cannot record feedback against agent B's prediction (validated against `RealityPrediction.session_id`) |
| Active-learning queue | shared queue; operator can label entries from any session; no per-session filtering needed |

---

## 9. Multi-node / multi-machine deployment (out of scope)

Per `docs/futurebuild/23 §6`: implicitly single-machine v1 deployment. Multi-node is future work; would require:

- Distributed RocksDB (or replication)
- Cross-machine witness chain consensus
- Shared embedder model store (CDN + SHA pinning)
- Cross-machine quota enforcement

Not blocking v1 (single-machine is the design point).

---

## 10. Tenant isolation (operator-level, BlackBoxFix)

The `blackbox` binary (separate from MCP daemon) is multi-tenant via `tenant_id`:

| Surface | Per-tenant |
|---|---|
| BlackBox ledger | per-tenant SQLite under `~/.blackbox/<tenant>/` |
| Episode export | per-tenant |
| Enterprise controls | per-tenant |
| SIEM export | per-tenant audit trail |

ME-JEPA itself is **single-tenant** (single operator hardware); tenancy is BlackBoxFix Phase 1 only.

---

## 11. Cross-references

[`13_data_flywheel.md`](13_data_flywheel.md) (shift subscriber), [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md) (per-tool isolation), [`24_security_adversarial.md`](24_security_adversarial.md) (rate limits + auth).
