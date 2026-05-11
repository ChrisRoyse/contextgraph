# TECH-LOOP — Technical Specification

> **Level 3.** Defines HOW the loop refactor + inner-loop residue cleanup is implemented. Implements [`SPEC-LOOP.md`](../functional/SPEC-LOOP.md).

---

## Touched files

```
.claude/loop.md                              # FULL REPLACE per 14_NEW_LOOP_MD.md
.claude/settings.json                        # extension: prediction-injection hook
CLAUDE.md                                    # §3, §11.11, §13 small edits
agents/README.md                             # mark retired agents
agents/cc-reality-harness-editor.md          # frontmatter status: retired
agents/cc-reality-diagnostician.md           # frontmatter status: retired
agents/cc-reality-reward-analyst.md          # frontmatter status: retired
scripts/cc/hook_post_tool_use_*.sh           # output paths → /mnt/d/contextgraph/runtime/cgreality-shift-log/
scripts/audit/no_inner_actor_models.sh       # NEW: confirms no qwen/hermes/ollama files
ci/lints/no_c_drive_paths.sh                 # NEW: lint
memory/decisions/                            # mark superseded with frontmatter status: superseded + superseded_by:
```

No code changes outside `.claude/`, `agents/`, `scripts/`, `ci/`, `memory/`. The Rust workspace is untouched by Phase H.

---

## `.claude/settings.json` extension

```jsonc
{
  "hooks": {
    "PreToolUse": {
      "Edit": ["scripts/cc/hook_pre_tool_use_edit.sh"],
      "Write": ["scripts/cc/hook_pre_tool_use_edit.sh"]
    },
    "PostToolUse": {
      "Edit": ["scripts/cc/hook_post_tool_use_edit.sh"],
      "Write": ["scripts/cc/hook_post_tool_use_edit.sh"],
      "Bash": ["scripts/cc/hook_post_tool_use_test_capture.sh"]    // NEW per Phase A
    },
    "UserPromptSubmit": [
      "scripts/cc/hook_user_prompt_submit_inject_prediction.sh"    // NEW per Phase A
    ],
    "SessionStart": ["scripts/cc/hook_session_start.sh"],
    "Stop": ["scripts/cc/hook_stop.sh"],
    "PreCompact": ["scripts/cc/hook_pre_compact.sh"]
  }
}
```

---

## Updated `scripts/cc/hook_post_tool_use_edit.sh` (D-drive paths)

```bash
#!/usr/bin/env bash
# scripts/cc/hook_post_tool_use_edit.sh (modified for D-drive paths)
set -euo pipefail

SHIFT_LOG_DIR="/mnt/d/contextgraph/runtime/cgreality-shift-log"
SESSION_ID="${CLAUDE_SESSION_ID:-default}"
SHIFT_LOG="$SHIFT_LOG_DIR/$SESSION_ID.jsonl"

mkdir -p "$SHIFT_LOG_DIR"

FILE="${CLAUDE_TOOL_INPUT_FILE_PATH:-}"
[[ -z "$FILE" ]] && exit 0

AFTER_SHA=$(sha256sum "$FILE" | awk '{print $1}')
LINE_COUNT=$(wc -l < "$FILE")

# cargo check for Rust files (release path)
CARGO_CHECK_RESULT="not_applicable"
if [[ "$FILE" == *.rs ]]; then
    if cargo check --release --message-format=short 2>&1 | head -50 > /tmp/cargo-check-out; then
        CARGO_CHECK_RESULT="ok"
    else
        CARGO_CHECK_RESULT="error"
    fi
fi

GIT_DIFF=$(cd "$(dirname "$FILE")" && git diff --stat "$FILE" 2>/dev/null | head -3)

# Append to shift log
jq -n \
    --arg ts "$(date -u +%FT%TZ)" \
    --arg session "$SESSION_ID" \
    --arg file "$FILE" \
    --arg after_sha "sha256:$AFTER_SHA" \
    --arg line_count "$LINE_COUNT" \
    --arg cargo "$CARGO_CHECK_RESULT" \
    --arg diff "$GIT_DIFF" \
    '{ts: $ts, session_id: $session, file: $file, after_sha: $after_sha, line_count: $line_count|tonumber, cargo_check: $cargo, git_diff_summary: $diff}' \
    >> "$SHIFT_LOG"

# Output the post-edit reality block (existing format)
echo "PostToolUse:Edit hook additional context: Post-edit reality: file=$FILE after_sha=sha256:$AFTER_SHA. Active-root source of truth: file:/mnt/d/contextgraph/state/cgreality/active_runtime_root."
```

---

## C-drive lint (CI)

```bash
#!/usr/bin/env bash
# ci/lints/no_c_drive_paths.sh
set -euo pipefail

VIOLATIONS=$(grep -rn -E '"/(home/|tmp/contextgraph-)' \
    --include='*.rs' --include='*.toml' --include='*.json' \
    crates/ scripts/ \
    | grep -v -E '(test|mock|fixture|memory/|docs/|\.claude/)' \
    | grep -v 'tempfile' \
    || true)

if [[ -n "$VIOLATIONS" ]]; then
    echo "FAIL: hardcoded C-drive paths detected. Move to /mnt/d/contextgraph/."
    echo "$VIOLATIONS"
    exit 1
fi
echo "PASS: no C-drive paths found."
```

Wired into CI as a step that runs before `cargo build`.

---

## Inner-actor model audit script

```bash
#!/usr/bin/env bash
# scripts/audit/no_inner_actor_models.sh
set -euo pipefail

PATTERNS=("*qwen*.gguf" "*hermes*.gguf" "*ollama*" "*llama-cpp*.so")

VIOLATIONS=()
for pat in "${PATTERNS[@]}"; do
    found=$(find /mnt/d /home -name "$pat" 2>/dev/null || true)
    if [[ -n "$found" ]]; then
        VIOLATIONS+=("$pat: $found")
    fi
done

if [[ ${#VIOLATIONS[@]} -gt 0 ]]; then
    echo "FAIL: inner-actor model artifacts present:"
    printf '  - %s\n' "${VIOLATIONS[@]}"
    exit 1
fi
echo "PASS: no inner-actor model files."
```

Run weekly via cron or as part of CI. Fail-closes if any reappear.

---

## SMAP supersession protocol

For each decision in the supersede list (per `11_DELETION_LIST_INNER_LOOP.md §4`):

```yaml
# memory/decisions/agent-77-harness-editor--inner-actor-model-swap-qwen3-coder-30b-a3b.md
---
namespace: decisions
agent_id: agent-77-harness-editor
timestamp: 2026-05-09T18:00:00Z
status: superseded                                    # ← changed from active
superseded_by: 2026-05-09--single-outer-loop-mejepa-no-local-llm.md   # ← NEW field
superseded_at: 2026-05-10T22:00:00Z                   # ← NEW field
tags: inner-actor, model-swap, RETIRED-PIVOT-2026-05-09
---

# Inner-Actor Model Swap: Qwen3-Coder-30B-A3B-MoE
[... original content unchanged ...]

> **SUPERSEDED 2026-05-10 PER POST-PIVOT CLEANUP.** The inner-actor concept is retired entirely; see superseded_by. This file is kept for provenance per CLAUDE.md §6.
```

---

## CLAUDE.md edits (precise diffs)

### §3 — add to directory layout

```diff
 ./scripts/     — utility scripts (cc/ holds Claude Code hook scripts)
 ./examples/    — example code
+./docs/futurebuild/ — forward plan: ME-JEPA-as-truth-system, single outer loop, agent-feedback channel
 ./tasks/       — workstream-specific task manifests (e.g. tasks/swebench-lite/*.json)
```

### §11.11 — mark `/ccreality-trigger-check` retired

```diff
 - `/ccreality-iterate` — run one full optimizer turn (the workhorse).
 - `/ccreality-smoke` — verify the full pipeline without Docker.
-- `/ccreality-trigger-check` — read-only check on whether to iterate.
+- `/ccreality-trigger-check` — RETIRED post-pivot; no inner-actor drip to trigger.
 - `/ccreality-readback` — drift detection on past recommendations.
+- `/loop` — canonical per-tick skill; see `docs/futurebuild/14_NEW_LOOP_MD.md`.
```

### §13 — add futurebuild reference

```diff
 - Per-tick prompt: `.claude/loop.md` (the canonical operational spec for one turn).
 - Agent definitions: `.claude/agents/cc-reality-*.md`; SMAP roster: `agents/README.md`.
+- Forward plan: `docs/futurebuild/00_INDEX.md` — superseding plan; CLAUDE.md remains the contract.
```

---

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `loop_md_replace_fsv` | head-50 of `.claude/loop.md` matches reference; no Mode A/B / inner LLM strings | inline grep checks |
| `c_drive_lint_fsv` | lint passes after migration | CI step output |
| `no_inner_actor_models_fsv` | audit script returns 0 violations | `/mnt/d/contextgraph/fsv/loop-fsv/no_inner_actor.json` |
| `smap_supersession_fsv` | every superseded decision has a `superseded_by:` pointer that resolves | `/mnt/d/contextgraph/fsv/loop-fsv/smap_supersession.json` |
| `claude_md_edit_fsv` | CLAUDE.md §3, §11.11, §13 contain the new lines | inline grep checks |
