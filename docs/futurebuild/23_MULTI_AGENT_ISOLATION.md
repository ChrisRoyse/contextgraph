# 23 — Multi-Agent Isolation

> **Purpose.** Specify how multiple AI agents (or multiple Claude Code sessions) sharing the same ME-JEPA daemon stay isolated, what's shared, and what's per-agent.

---

## 1. Isolation model

| Resource | Scope | Notes |
|---|---|---|
| Predictor weights | shared (active model) | one model serves all agents |
| Constellation centroids | shared | per-(cat × ent × lang × emb), agent-agnostic |
| Conformal calibration | shared | global + per-cell |
| `CF_MEJEPA_PANELS` | per-session | keyed by `(session_id, panel_id)` |
| `CF_MEJEPA_LIVE_PREDICTIONS` | per-session | keyed by `session_id`; agent only sees its own |
| `CF_MEJEPA_AGENT_FEEDBACK` | per-(agent_id, prediction_id) | each agent's events isolated |
| `CF_MEJEPA_TRAIN_CERTS` | shared | training affects everyone (the global model) |
| Active-learning queue | shared | operator-managed; entries reference originating session |
| Drift window | shared | global model drift |
| Sampling weights | shared | bumps apply globally |
| Shift-log files | per-session | separate JSONL per `session_id` |
| Witness chain | shared (single chain per machine) | session_id is part of each entry's metadata |

**The model is one global resource serving all agents; data is per-session except where it feeds the global model.**

---

## 2. Session ID derivation

Per `.claude/settings.json` and existing hooks: `session_id` is derived from the Claude Code session UUID. For non-Claude-Code clients (e.g., a CI bot calling `mejepa_verify`), the client provides `session_id` in the call args.

If absent, the daemon assigns a UUID. Sessions are isolated by `session_id`; collisions are extremely unlikely.

---

## 3. Concurrency

### 3.1 Concurrent predictions

Multiple agents can call `mejepa_verify` concurrently. The CUDA queue serializes the predictor forward pass. p99 latency widens proportionally to active concurrency:
- 1 active agent: p99 ≈ 100ms
- 5 active agents: p99 ≈ 500ms
- 10 active agents: p99 ≈ 1s

This is acceptable for the use case.

### 3.2 Concurrent shifts

Each session writes to its own JSONL file under `runtime/cgreality-shift-log/<session>.jsonl`. The subscriber tails all files in parallel. No cross-session contention.

### 3.3 Concurrent edits to the same file from different sessions (D11)

Risk: two agents edit the same file in different sessions; the on-disk state is non-deterministic from either agent's view.

Mitigation:
- The shift-log entry records the file path + before_sha + after_sha. If the before_sha doesn't match the after_sha of the most recent shift for the same file, mark the shift as `external_drift_detected: true`.
- The subscriber materializes the panel from the actual file content (post-edit). Concurrent edits produce two panels for the same file at different SHAs; both are correct (each at a different point in time).
- Agents that detect external drift via `mcp__cgreality__reality_shift_compare_to_my_view` re-perceive before continuing.

This is the same reality-shift principle from CLAUDE.md §11.5; it just needs to handle the multi-session case explicitly.

### 3.4 Concurrent feedback events

`mejepa_record_agent_feedback` writes to `CF_MEJEPA_AGENT_FEEDBACK` keyed by `(prediction_id, agent_id, ts)`. RocksDB writes are atomic. No locking needed.

---

## 4. Feedback volume behavior

Per the local-operator contract, `mejepa_record_agent_feedback` is not rate-limited by `agent_id`. Valid feedback must be durably persisted; invalid feedback must fail closed with explicit validation/storage context.

Other tools are also not rate-limited per agent because they're either pure reads or naturally bounded by compute/storage capacity. Isolation remains session-based: state writes include `agent_id` where relevant, but cross-session reads require an explicit session identifier.

---

## 5. Cross-session prediction visibility

By default: an agent's `mejepa_predict_latest` returns only its own session's prediction.

Operator override: `mejepa_predict_latest --session-id <other-session>` returns another session's prediction. Use case: operator inspecting a specific agent's reality without sitting in that agent's session. Not exposed to agents.

---

## 6. Cross-machine deployment (D20)

The current design is **single-machine, single-daemon, multi-agent**. Cross-machine deployment is out of scope for v1. If an operator wants ME-JEPA shared across multiple developer machines:

Option A — Centralized daemon, distributed clients:
- One daemon runs on a designated host
- Developer machines connect to it via TCP MCP (operator-enabled)
- Risks: network latency in shift propagation; secrets policy (D-drive sharing)

Option B — Per-machine daemon, federated training:
- Each developer machine runs its own daemon
- Periodic federated training merges weights (out of scope per `17 §O3`)

Both options are deferred. v1 assumes one machine per operator.

---

## 7. Authentication and authorization

Default: no auth. The MCP unix socket is access-controlled by file permissions (`chmod 600`). Anyone with operator-user privileges can call any tool.

Optional: TCP exposure with per-token auth (operator-enabled config). Out of scope for v1; if an operator opts in, they should also enable TLS + per-tool ACL.

---

## 8. Storage isolation per session

Per `06 HYGIENE` (shipped):
- Per-session shift-log files have independent retention windows
- Per-session predictions in `CF_MEJEPA_LIVE_PREDICTIONS` TTL after 30 days
- Per-session test outcomes are tagged with session_id for accountability

When a session ends (Claude Code session closes), no explicit cleanup is needed — TTL handles it. Operator can force cleanup via `context-graph-cli mejepa session cleanup --session-id <id>`.

---

## 9. What this commits us to

- `TASK-MAI-001` — multi-agent E2E test (in `18_INTEGRATION_AND_E2E_TESTING.md §2.3`; covered by `TASK-TEST-003`)
- `TASK-MAI-002` — concurrent-edit external-drift detection (extension to existing `reality_shift_compare_to_my_view`)
- `TASK-MAI-003` — `context-graph-cli mejepa session cleanup` command
- `TASK-MAI-004` — operator-mode `--session-id` flag for `mejepa_predict_latest`

These are small extensions; multi-agent isolation is mostly an emergent property of the per-session storage design that's already in place.
