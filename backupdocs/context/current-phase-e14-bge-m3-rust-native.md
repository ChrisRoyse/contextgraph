---
namespace: context
agent_id: coordinator
timestamp: 2026-04-22T15:00:00Z
status: active
tags: phase-1, e14, bge-m3-dense, rust-native, embedder-integration
---

# Current Phase: E14 BGE-M3 Dense — Full Rust-Native Integration

## Mission
Integrate BAAI/bge-m3 (dense output head only) as ModelId::BgeM3Dense (E14)
into the context-graph Rust daemon at full parity with existing E1-E13
embedders. Native candle implementation, no Python sidecar, no ONNX, no
fallbacks.

## Authority docs (read these first)

- `docs/embedadd/README.md` — master index of planning docs
- `docs/embedadd/00_strategy.md` — 6-phase strategy
- `docs/embedadd/01_reference_patterns.md` — 7-layer integration anatomy
- `docs/embedadd/02_touchpoints.md` — all files that must change
- `docs/embedadd/03_pilot_bge_m3_dense.md` — Phase 1 spec
- `docs/embedadd/08_storage_layer.md` — CF + HNSW schemas
- `docs/embedadd/09_mcp_surface.md` — MCP tool changes
- `docs/embedadd/10_fsv_protocol.md` — 6-check FSV
- `docs/embedadd/11_reingestion.md` — migration plan
- `docs/embedadd/12_task_breakdown_pilot.md` — 23 atomic tasks (THIS IS THE WORK LIST)
- `/home/cabdru/leapablememory/docs2/SHARED-MIND-AGENT-PROTOCOL.md` — SMAP
- `/home/cabdru/datalab/docs2/borisflow.md` — workflow rules

## Hard constraints (user directives, non-negotiable)

1. **NO workarounds, NO fallbacks.** Fail-fast with structured logging.
2. **NO mock data in tests.** Real HF weights, real Sonnets, real daemon.
3. **NO tests that pass when broken.** If the model isn't loading, the
   test fails; don't degrade assertions.
4. **NO backwards compatibility.** NUM_EMBEDDERS flips 13→14 cleanly. Any
   consumer that expects 13 must be updated in this phase.
5. **Full State Verification mandatory** at end of phase.
6. **Subagents synchronous** with memory handoff via SMAP.
7. **Coordinator verifies every subagent deliverable** before advancing.
8. **code-simplifier review** at end.

## Success criteria (from 03_pilot_bge_m3_dense.md DoD)

- All 7 layers integrated (ModelId, model impl, factory, storage CF, HNSW, MCP, tests)
- `cargo check --workspace` passes
- `cargo test --workspace` passes no regressions
- 6-check FSV passes
- Python parity: cos ≥ 0.995 on 10 Sonnets vs HF reference
- Reingestion of all 2,770+ records successful
- `search_by_embedder(embedder="E14")` works via MCP
- `score_against_constellation` returns 14-element per_embedder_cosine
- Docs updated
- code-simplifier review completed

## Environment state at phase start

- Git branch: main (clean except docs/embedadd/ pending commit + model files from v4.2 session)
- GPU: RTX 5090, ~29 GB VRAM free after killing v4.2 inference server
- Disk: 175 GB free at /
- Daemon: running on :3100 (may need restart after CF changes)
- LMDB sidecar (v4.2 Python-side): 979 records — we compare against this for parity
- v4.2 DPO adapter: trained, saved, unchanged by this phase

## Agent pipeline (sequential, dependency-ordered)

This phase uses sequential subagents (not parallel) because each task has
hard dependencies on earlier work.

| # | Agent ID | Role | Tasks | Depends on |
|---|----------|------|-------|------------|
| 1 | agent-e14-1-tokenizer-probe | verification | Task 1 (verify XLM-RoBERTa tokenizer support in Rust) | Nothing |
| 2 | agent-e14-2-enum-plumbing | coder | Tasks 2-4 (TokenizerFamily + ModelId variant + NUM_EMBEDDERS) | Agent 1 |
| 3 | agent-e14-3-model-impl | coder | Tasks 5-12 (bge_m3_dense module + forward pass + smoke test) | Agent 2 |
| 4 | agent-e14-4-storage-mcp | coder | Tasks 14-17 (CF, HNSW, MCP DTOs, MCP handlers) | Agent 3 |
| 5 | agent-e14-5-migration | coder | Tasks 18-19 (migration CLI + run on live DB) | Agent 4 |
| 6 | agent-e14-6-python-parity | tester | Task 13 (Python parity test) | Agent 3 (can run in parallel with 4/5) |
| 7 | agent-e14-7-fsv | sherlock-holmes | Task 20 (6-check FSV) | Agent 5 |
| 8 | agent-e14-8-consumer-updates | coder | Task 21 (Python consumer updates) | Agent 4 |
| 9 | agent-e14-9-simplifier | code-simplifier | Task 22 (review) | Agent 7 |
| 10 | agent-e14-10-docs | coder | Task 23 (doc updates) | Agent 9 |

Coordinator (me) runs between each agent:
- Verifies all deliverables in the filesystem / DB
- Reads their SMAP memory writes
- Runs acceptance criteria myself
- Only advances if all green

## Memory trail conventions

Agents write to:
- `memory/decisions/agent-e14-{N}-*--*.md`
- `memory/discoveries/agent-e14-{N}-*--*.md`
- `memory/journal/agent-e14-{N}-*--{success|failure}-*.md`
- `memory/blockers/agent-e14-{N}-*--*.md` (severity: high if blocks phase)
- `memory/handoff/agent-e14-{N}-to-e14-{N+1}--*.md`
- `memory/reports/agent-e14-{N}-*--final-report.md`

Every agent's final-report MUST list all memory files written + files created/modified in repo. Coordinator greps for those paths to verify.

## Coordinator's pre-flight checklist

- [x] Back up `contextgraph_data/` to `contextgraph_data.pre_e14.bak` (will do before Task 14)
- [x] VRAM free ≥ 10 GB (verified, 29 GB free)
- [x] `cargo build --workspace` baseline (running)
- [x] `cargo test --workspace` baseline (will run before any changes)
- [ ] HF_TOKEN env valid (will verify in Task 1)
