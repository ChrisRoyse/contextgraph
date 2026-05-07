---
namespace: context
agent_id: coordinator-exp2-verification
timestamp: 2026-04-28T22:50:00Z
status: active
tags: exp2, verification, mission, mutual-information, n-eff
---

# EXP-2 Verification Campaign — Mission Brief

## Goal

Operator (chrisroyseai@gmail.com) has invoked a full state verification pass on the EXP-2 implementation in `scripts/exp2/` against the spec docs in `docs/exp2/00..09`. The operator's hard requirements:

1. Verify exp2 was fully implemented (or expose the gap with evidence).
2. Full State Verification — read the source-of-truth (RocksDB CFs, on-disk artefacts, test outputs), do not trust return values alone.
3. **No workarounds, no fallbacks, no mocks.** If something doesn't work, it must fail loud with diagnostics.
4. **No backwards compatibility.** Pre-production. Schema changes wipe state.
5. Use synthetic data we know the answer to (e.g. synthetic Gaussians with closed-form MI) — and verify outputs match.
6. Operator will personally check sub-agent outputs; agents must report which memory files they wrote.

## What EXP-2 Is

A Python harness (~13 files in `scripts/exp2/`) measuring pairwise mutual information across $\binom{14}{2} = 91$ embedder pairs of the Context Graph production panel. Replaces paper headline "up to 91 signals per input" with empirically measured $N_{\mathrm{eff}}$. See `docs/exp2/00_README.md` for full goal.

Pipeline: corpus (8 strata, 800 inputs) → 14-embedder extraction via real MCP server → PCA-32 projection → InfoNCE+KSG MI per pair → bootstrap CIs → 3 effective-rank methods → heatmap + audit chain + FSV replay.

## Reality Check At Campaign Start (2026-04-28)

| Check | State | Evidence |
|---|---|---|
| Code present | YES — 13 .py files + 4 test files | `scripts/exp2/` |
| Unit tests pass | YES — 7 passed | `tasks/exp2/todo.md` line 45 |
| Manual FSV ran via real MCP+RocksDB | YES — 3 fingerprints, 3 training records, 3 edge cases | `artefacts/exp2/manual_fsv_log.json` |
| T-001 env verifier honest | YES — fails on 17,639 MiB free < 30 GiB required | `artefacts/exp2/verification_log.json` |
| Production pipeline run | **NO** — blocked at T-001 by GPU memory | `tasks/exp2/todo.md` |
| Corpus manifest exists | NO | no `artefacts/exp2/corpus_manifest_v1.json` |
| Embeddings extracted | NO | no `embeddings_per_input.npz` |
| 91 pair results | NO | no `pair_results/*.json` |
| Heatmap | NO | no `redundancy_heatmap.{png,pdf}` |
| N_eff summary | NO | no `n_eff_summary.json` |
| FSV replay | NO | only manual_fsv (storage layer), not the full pipeline |

The implementation **author was honest**: todo.md cleanly distinguishes "completed" (unit-testable, runnable) from "implemented, not run" (waits on T-007 corpus). The blocker is real: only ~17 GiB GPU free.

## What Verification CAN Do Without GPU

- Static audit of every Python module against spec.
- Pytest suite (CPU + small synthetic).
- T-013 synthetic ground-truth validation (CPU Gaussians; no real embedders).
- T-001 verifier re-run (must fail honestly per Constitution C16).
- Manual FSV via running MCP daemon (already on port 3100).
- Build corpus strata that don't need internet (S1 Shakespeare, partial S2 code).
- Confirm artefacts directory state matches the honest claims.

## What Verification CANNOT Do Without GPU

- T-007 (extract 800 × 14 embeddings) — requires 30 GiB free VRAM.
- T-016/17/18 (91 InfoNCE training runs + KSG + bootstrap) — requires real projected embeddings.
- T-021..T-025 (N_eff, heatmap, FSV replay on full pipeline).

The right answer to the operator is **honest verification**: confirm what works, expose what is blocked, propose nothing the GPU constraint allows us to test.

## SMAP Discipline For This Campaign

- This file: project root `./memory/` (i.e. `/home/cabdru/contextgraph/memory/`).
- Subdirs already exist: context, decisions, discoveries, handoff, journal, blockers, patterns, reports.
- Each agent reads `./memory/context/exp2-mission.md` (this file) first.
- Agents announce every memory file they write at the end of their report.
- The coordinator (me) verifies each subagent's outputs by reading the listed files.

## Agents Planned (Sequential)

| # | Agent | Subagent type | Mission |
|---|---|---|---|
| 01 | static-audit | code-analyzer | Static audit of every scripts/exp2/*.py against spec — identify mocks, fallbacks, fail-fast gaps |
| 02 | tester | tester | Run pytest + synthetic Gaussian sweep + edge cases. Verify outputs match closed-form MI |
| 03 | sherlock | sherlock-holmes | Live forensic: rerun verifier, manual_fsv, corpus build, inspect RocksDB CFs |
| 04 | simplifier | code-simplifier | Review agents 1-3 findings, propose refinements, ensure elegance |

After all 4: coordinator does personal verification + final synthesis.
