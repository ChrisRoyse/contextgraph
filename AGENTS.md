# AGENTS.md — ContextGraph

**Operating contract (universal):** [`docs2/AICodingAgentSuperPrompt.md`](docs2/AICodingAgentSuperPrompt.md) — the AI Coding Agent Doctrine. Master rules for any AI agent in this repo. Read once at session start.

**Project addendum:** [`CLAUDE.md`](CLAUDE.md) — ContextGraph / ME-JEPA-Code specifics: invariants, crates, paths, deny-list, build commands.

**Per-tick prompt:** [`.claude/loop.md`](.claude/loop.md) — what to do each `/loop` firing.

When the doctrine and `CLAUDE.md` agree, follow either. When they conflict, **the doctrine wins** for protocol detail (memory, claiming, FSV, RCA, hardening, communication, session lifecycle) and `CLAUDE.md` wins for project-specific invariants (paths, crates, deny-list, current phase).

---

## Memory = GitHub Issues

Per doctrine §3, all institutional memory lives in this repo's GitHub Issues. There is **no** `./memory/` directory. Old SAMP/SMAP content is archived at `./memory.archive/` (read-only history; do not write new files there).

Read at the start of every turn (doctrine §3.3):

```bash
# 1. Pinned mission / current phase
gh issue list --state open --label "type:context" --json number,title,body

# 2. In-progress (avoid stepping on others)
gh issue list --state open --label "status:in-progress" --json number,title,assignees,updatedAt

# 3. Blocked (may be pickup-able if unblocked)
gh issue list --state open --label "status:blocked" --json number,title,assignees

# 4. Unclaimed queue (your candidates)
gh issue list --state open --label "source:agent" --search "no:assignee" --json number,title,labels,updatedAt

# 5. Active decisions binding you (closed = locked in)
gh issue list --state closed --label "type:decision" --limit 20

# 6. Discoveries / patterns touching your task area
gh issue list --state closed --label "type:discovery,type:pattern" --search "<keywords>" --limit 20
```

Then read `docs/futurebuild/README.md` for the current 8-doc map, plus `docs/futurebuild/01_overview.md`, `07_results_and_benchmarks.md`, and `08_roadmap_and_open_work.md` for current build state and open work.

---

## Two non-negotiable coordination rules (doctrine §3.2)

1. **File rule.** Defect / smell / anomaly / decision / discovery / pattern you are NOT capturing in code this turn → open a GitHub Issue before turn ends. If it isn't tracked, it dies with the session.
2. **Claim rule.** Before touching code tied to an Issue → assign self, flip `status:needs-triage` → `status:in-progress`, post a CLAIM comment with files-you-will-touch and ETA. Comment at every milestone. End the turn with RESOLVED / PAUSE / BLOCKED on every issue you worked.

No silent work. No `./memory/*.md` writes.

---

## Labels (bootstrap once)

Label set defined in [`.github/labels.yml`](.github/labels.yml). Apply with:

```bash
scripts/gh-labels-bootstrap.sh
```

Cap per issue: 1 `type:*` + 1 `priority:*` (default `priority:p2`) + 1 `status:*` + 1-2 `area:*` + `source:*` + `agent:*`.

---

## Project quick-reference

- **What:** ME-JEPA-Code — deterministic predictive truth system for AI-generated code changes.
- **Ship gate:** `prediction_oracle_correlation ≥ 0.95` stably across 4 rolling windows on 300×8 SWE-bench Lite.
- **Workspace:** Rust, 27+ crates under `crates/`.
- **Build:** `cargo build --release -p <crate>` → `cargo test -p <crate>` → `cargo clippy -p <crate> --no-deps --tests --examples -- -D warnings`.
- **aiwonder production SoT:** post-migration durable corpus/runtime/project/FSV state lives on aiwonder under `/zfs/archive/contextgraph/`; hot work lives under `/zfs/hot/contextgraph/` or explicitly budgeted aiwonder scratch. Local `/mnt/d/contextgraph/` references are legacy/local evidence unless an issue explicitly requests a local-only smoke. See #363.
- **MCP server:** the only network surface for runtime work. No HTTP from action paths.
- **No inner LLM.** Retired 2026-05-09. Do not reanimate without explicit operator authorization.

Full detail in `CLAUDE.md` and `docs/futurebuild/README.md`.
