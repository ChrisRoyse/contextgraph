# v4.2 Extended Ensemble — Todo

## Status Key
- [ ] todo
- [~] in-progress
- [x] done
- [!] blocked/failed (see memory/blockers/)

## Phase 1 — Coordinator bootstrap (DO FIRST)
- [x] Read SMAP protocol, borisflow, existing memory trail
- [x] Write `memory/context/current-phase-v4_2-extended-ensemble.md`
- [x] Write this `tasks/v4_2_extended_ensemble_todo.md`

## Phase 2 — Subagent 1: Extended Ensemble Builder
- [ ] Agent spawns, reads memory, builds `scripts/v4_2/extended_ensemble.py`
  - Loads BGE-M3 via sentence-transformers or transformers direct
  - Loads Jina-embeddings-v3 with `classification` task LoRA
  - Loads Qwen3-Embedding-0.6B with task-prompt wrapper
  - Loads BGE-reranker-v2-m3 as cross-encoder
  - Every load path: ERROR if fail — no silent degradation
  - Smoke-test main emits 5-embedder scores for Shakespeare vs modern
- [ ] Agent writes `memory/discoveries/agent-v42-1-ensemble-builder--*.md`
- [ ] Agent writes `memory/handoff/agent-v42-1-to-v42-2--*.md`
- [ ] Agent writes `memory/reports/agent-v42-1-ensemble-builder--final-report.md`
- [ ] Coordinator verifies: file exists, smoke test passed, VRAM budget OK

## Phase 3 — Subagent 2: Sonnets Reference Builder
- [ ] Agent reads handoff from Agent 1
- [ ] Builds `scripts/v4_2/build_references.py`:
  - Queries Sonnets constellation members (313 records) via daemon
  - Computes each member's embedding through all 5 new embedders
  - Aggregates into reference centroid + covariance
  - Persists to `~/.contextgraph/models/extended_ensemble/sonnets_references.npz`
  - Includes provenance metadata (model versions, compute timestamp, member count)
- [ ] Agent writes discoveries + handoff + final-report to memory
- [ ] Coordinator verifies: npz file exists, contains expected keys, centroid
  dimensions match model outputs

## Phase 4 — Subagent 3: Integrator
- [ ] Agent updates `scripts/v4_2/camd_lib.py`:
  - Adds `ExtendedConstellationScorer` class combining daemon-13 + ensemble-5
  - Adds extended reward function (weighted combination of 18 views)
  - NO BACKWARDS COMPATIBILITY — drops old score() if interface changes
  - ERROR OUT if ensemble references not found
- [ ] Updates `mine_camd_pairs.py` to use extended scorer
- [ ] Smoke test: re-mine 5 CAMD pairs using extended reward, verify
  stronger style-discrimination than daemon-only
- [ ] Agent writes discoveries + handoff + final-report
- [ ] Coordinator verifies: code compiles, smoke test produces valid pairs
  with improved gap metric

## Phase 5 — Subagent 4: Verifier (sherlock-holmes)
- [ ] Agent reads all prior memory
- [ ] Full State Verification:
  - Model files exist on disk with correct sizes
  - References.npz content matches expected schema
  - Daemon returns correct constellation (physical check via RPC)
  - Extended scorer produces deterministic outputs across 5 runs
  - Real Shakespeare vs real modern discrimination gap ≥ 0.20 combined
- [ ] Edge-case audit:
  - Empty text → errors cleanly
  - Non-English text → scorer handles (or explicit error)
  - >8K token text → errors cleanly
  - Missing reference file → errors cleanly
- [ ] Agent writes discoveries + final-report with physical evidence

## Phase 6 — Subagent 5: code-simplifier
- [ ] Reviews all 3 new files + camd_lib updates
- [ ] Applies refinements per patterns in `memory/patterns/`
- [ ] Writes final-report

## Phase 7 — Coordinator wrap-up
- [ ] Read all 5 final-reports, confirm "Task Completed: yes" each
- [ ] No unresolved HIGH blockers
- [ ] Write coordinator final-report

## Global invariants (enforce throughout)
- Every failure gets a memory/journal/agent-*-failure-*.md
- Every fix gets a memory/journal/agent-*-success-*.md
- No fallback code paths — error-out + log
- No mock data — real Shakespeare + real modern samples only
- Subagent outputs verified by coordinator before next spawn
