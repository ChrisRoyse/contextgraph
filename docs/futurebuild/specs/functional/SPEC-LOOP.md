# SPEC-LOOP — Functional Specification

> **Level 2.** Defines WHAT the refactored loop and the inner-loop residue cleanup must do.
> Owns: REQ-LOOP-01..12.
> Implementation reference: [`06_REALITY_LOOP_REFACTOR.md`](../../06_REALITY_LOOP_REFACTOR.md), [`14_NEW_LOOP_MD.md`](../../14_NEW_LOOP_MD.md), [`11_DELETION_LIST_INNER_LOOP.md`](../../11_DELETION_LIST_INNER_LOOP.md).

---

## User stories

- **As an operator running `/loop`**, I want a single canonical per-tick procedure that picks the highest-priority work without me having to choose between modes.
- **As Claude Code (the agent)**, I want clear orientation reads at turn start so I can pick the right work item.
- **As a maintainer**, I want the inner-loop residue removed so the codebase is honest about what's running.

## Requirements

### REQ-LOOP-01 — `.claude/loop.md` content replacement

The literal content of `.claude/loop.md` SHALL be replaced with the block specified in `14_NEW_LOOP_MD.md` between the `=== BEGIN LOOP MD ===` and `=== END LOOP MD ===` markers.

### REQ-LOOP-02 — Single work-class priority pick

The new loop SHALL pick exactly one work item per turn from priority order: (1) daemon health remediation if degraded, (2) Phase A futurebuild tasks until done, (3) Phase B/C/D/E/F futurebuild tasks, (4) Phase G capture work in parallel, (5) Phase H cleanup as filler.

### REQ-LOOP-03 — Mode A/B retired

The new loop SHALL NOT reference "Mode A" / "Mode B". The bipartite framing is retired.

### REQ-LOOP-04 — Daemon-health check at orientation

The new loop SHALL include a daemon-health check (`mejepa_subscriber_status`, `mejepa_heal_status`, `mejepa_quota_status`) in the per-turn orientation step. Degraded health takes precedence over the highest-priority task.

### REQ-LOOP-05 — D-drive paths in orientation reads

The new loop's orientation step SHALL reference D-drive paths for shift logs (`/mnt/d/contextgraph/runtime/cgreality-shift-log/`) and storage (`/mnt/d/contextgraph/storage/contextgraph-rocksdb/`). C-drive paths in the loop content are forbidden.

### REQ-LOOP-06 — Hook output paths to D-drive

The PostToolUse hook script `scripts/cc/hook_post_tool_use_*.sh` SHALL write shift-log entries to `/mnt/d/contextgraph/runtime/cgreality-shift-log/<session>.jsonl`, NOT `/tmp/contextgraph-rust-reality-loop/cgreality-shift-log/`.

### REQ-LOOP-07 — CLAUDE.md updates

`CLAUDE.md` SHALL be updated:
- §3 (Directory layout): add `docs/futurebuild/`.
- §11.11 (Operator-driven skills): mark `/ccreality-trigger-check` as retired; mark `/loop` as canonical.
- §13 (Support): add reference to `docs/futurebuild/` as the canonical forward-work plan.

### REQ-LOOP-08 — `agents/README.md` updates

The SMAP roster SHALL mark `cc-reality-harness-editor`, `cc-reality-diagnostician`, `cc-reality-reward-analyst` as `status: retired` (keeping the agent files as historical reference per CLAUDE.md §6).

### REQ-LOOP-09 — Supersede SMAP decisions

Specific decisions from `memory/decisions/` referencing inner-actor concepts SHALL be marked `status: superseded` with a `superseded_by:` pointer to the corresponding futurebuild decision (per `11_DELETION_LIST_INNER_LOOP.md §4`). Files are NEVER deleted.

### REQ-LOOP-10 — Retire legacy MCP tools

MCP tools that map to inner-actor concepts SHALL be marked retired (already done for `mcp__cgreality__reality_run_attempt`); confirm no new tools introduce inner-actor language.

### REQ-LOOP-11 — Audit absence of inner-actor model files

A small audit script SHALL confirm that no GGUF / safetensors files matching `*qwen*` / `*hermes*` / `*ollama*` exist on disk (under any path). Fails CI if any reappear.

### REQ-LOOP-12 — Stale binary cleanup

Any lingering `target/release/reality-loop` build artifact SHALL be deleted on the next clean rebuild. Cargo workspace must NOT have a `[[bin]]` entry for `reality-loop`.

## Acceptance criteria

1. `head -50 /home/cabdru/contextgraph/.claude/loop.md` matches the head-50 of the block in `14_NEW_LOOP_MD.md`.
2. `grep -r "Mode A" .claude/loop.md` returns 0 matches; same for `Mode B`, `inner LLM`, `cc-reality-harness-editor`.
3. CLAUDE.md §3 contains `docs/futurebuild/`; §11.11 marks `/ccreality-trigger-check` as retired.
4. SMAP supersession is consistent: every superseded decision has a `superseded_by:` pointer; the pointed-to decision exists.
5. `find . -name "*qwen*.gguf" -o -name "*hermes*.gguf"` returns 0 matches.
6. `cargo metadata --format-version 1 | jq '.packages[].targets[] | select(.kind[] == "bin") | .name'` does NOT contain "reality-loop".

## Edge cases

- **`.claude/loop.md` already reflects new content**: idempotent — re-running TASK-LOOP-001 is safe.
- **Operator runs loop on a fresh clone with no `./memory/`**: bootstrap path per loop.md §9 fires; recommendation is `changed` (initial SMAP files written).
- **`/mnt/d/contextgraph/` not mounted**: loop's orientation fails closed; recommendation is `blocked` with `surface_to_operator: "D-drive root missing"`.

## Non-functional

- Loop turn end-to-end (read SMAP + execute work + record) target: 5-30 minutes for build work; up to 1-hour wall for capture work.
- Loop must be idempotent on retry: re-running on the same input state should produce a `no_change` recommendation if no work has shifted.
