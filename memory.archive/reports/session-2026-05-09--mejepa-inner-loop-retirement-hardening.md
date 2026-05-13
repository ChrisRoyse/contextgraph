---
namespace: reports
created: 2026-05-09T06:18:25Z
updated: 2026-05-09T06:22:55Z
status: active
tags: report, session, mejepa, hardening, fsv, local-llm-retirement, swe-bench
---

# Session Report: 2026-05-09 — ME-JEPA Inner-Loop Retirement Hardening

## Task
Review today's git history and current worktree, remove/harden retired local LLM and inner-loop surfaces, align docs around the single Claude Code outer loop, and verify by Full State Verification.

## Outcome
Complete and verified for the active build/tooling surface. The current FSV evidence is `/tmp/contextgraph-mejepa-retirement-fsv/run.log`, rerun after concurrent workspace churn settled.

## Summary
Today's git history showed the ME-JEPA pivot happened in staged commits: docs were refactored first, the `reality-loop`/vendored llama stack was removed next, and MCP/run/tooling cleanup followed. The root cause was architectural drift: the repo still carried a retired Claude-plus-local-Qwen/Hermes inner actor while the intended product is now a single outer Claude Code loop that captures durable reality evidence for ME-JEPA.

The hardening pass removed remaining active references to retired local-model/run tools from MCP startup, handler wiring, causal-discovery tool exposure, graph/typed-edge LLM validation paths, and active `.claude/agents` instructions. The planning docs were corrected where they still treated `reality_run_*` as future integration points. The retirement FSV script now scans `.claude/agents` too, so stale agent instructions are part of the source-of-truth verification surface.

Web research was used to sanity-check the design direction: JEPA systems predict latent representations rather than reconstructing raw inputs; VICReg-style variance/covariance checks are a known collapse defense; SWE-bench's real GitHub issue/oracle framing matches the intended durable-reality label source; and model-collapse/plasticity literature supports accumulating real readback evidence instead of recursive model-generated replacement data.

## Root Cause
The repo had two incompatible systems alive at the same time:

- Retired local LLM/inner actor: `reality-loop`, `llama-cpp-2`, Qwen/Hermes/Ollama-style generator paths, and MCP `reality_run_*` entrypoints.
- Intended ME-JEPA system: one outer Claude Code loop producing source-of-truth artifacts, tests, oracle verdicts, witness rows, and reward labels for a non-generative predictor/verifier.

That conflict caused build and verification risk: unrelated checks could pull native llama/CUDA builds, tool docs could steer agents toward removed MCP entrypoints, and future planning docs could accidentally reintroduce the inner actor.

## Files Created or Modified in the Repo
- `.claude/agents/cc-reality-*.md` — active agent instructions no longer mention retired `reality_run_smoke`, `reality_recompile_reward`, or inner-actor wording.
- `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` — Phase 7 now describes outer-loop ME-JEPA observation and verification.
- `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` — `/loop` semantics now use the current SWE-bench harness/oracle path and explicitly avoid retired `reality_run_*` tools.
- `scripts/cc/tests/mejepa_retirement_fsv.sh` — source-of-truth grep now includes `.claude/agents` so stale agent instruction references fail FSV.
- MCP/core/storage files in the current diff — active local-LLM causal discovery, graph discovery, LLM hint, and LLM edge-validation surfaces were removed or retired from tool exposure.

## Files Written or Updated in ./memory/
- `decisions/2026-05-09--single-outer-loop-mejepa-no-local-llm.md` — added SAMP frontmatter and retained the single-loop/local-LLM retirement decision.
- `blockers/2026-05-09--context-graph-cli-tests-blocked-by-vendored-llama-deletions.md` — resolved earlier; verification points at the retirement FSV log.
- `blockers/2026-05-09--go-harness-rerun-blocked-by-typescript-compile-error.md` — resolved earlier; CLI library check now passes.
- `reports/session-2026-05-09--mejepa-inner-loop-retirement-hardening.md` — this report.

## Decisions Made
- The only supported loop is the outer Claude Code loop that feeds ME-JEPA with durable readback evidence.
- The retired local LLM/inner actor must fail closed by absence, not fallback to another generator.
- Active FSV must include instruction surfaces, not only Rust source and manifests.

## FSV Evidence
Source of truth:

- Filesystem: `crates/context-graph-cli/src/bin/reality-loop.rs`, `vendor/llama-cpp-sys-2-0.1.145/`, `config/ccreality_model_pins.json`, `models/`, and target `reality-loop` binaries are absent.
- Active symbol readback: no retired local LLM or `reality_run_*` symbols in active CLI/MCP/tooling surfaces or `.claude/agents`.
- Build readback: `cargo check -p context-graph-cli --no-default-features --lib --quiet` passed.
- Build readback: `cargo check -p context-graph-mcp --no-default-features --lib --quiet` passed.

Before:

- The FSV run inspected absence of retired files/artifacts and searched active build/tooling/instruction surfaces for retired symbols.

Trigger:

```bash
rm -rf /tmp/contextgraph-mejepa-retirement-fsv && OUT=/tmp/contextgraph-mejepa-retirement-fsv bash scripts/cc/tests/mejepa_retirement_fsv.sh
```

After:

```text
=== FSV PASS ===
after.source_of_truth.files_absent=reality-loop.rs,vendor/llama-cpp-sys-2-0.1.145,config/ccreality_model_pins.json,models,target reality-loop binaries
after.source_of_truth.build=cargo-check-cli-lib,cargo-check-mcp-no-default
after.source_of_truth.retired_symbols=retired-active-references.txt,retired-tool-*.txt,retired-spawn-readback.txt
```

## Edge Cases Manually Verified
- Retired binary target fails closed: `cargo check -p context-graph-cli --bin reality-loop --quiet` fails with the expected missing-bin error.
- MCP cannot expose retired runner tools or spawn stale binaries: retired tool-name checks and stale spawn-path grep passed, then MCP no-default library check passed.
- Hooks/benchmarks do not target retired binaries or local LLM bins: readback showed CLI library checks only and no retired runner targets.

## Research Sources Used
- I-JEPA: https://arxiv.org/abs/2301.08243
- V-JEPA: https://arxiv.org/abs/2404.08471
- VICReg: https://openreview.net/forum?id=xm6YD62D1Ub
- SWE-bench: https://arxiv.org/abs/2310.06770 and https://github.com/SWE-bench/SWE-bench
- Model collapse: https://www.nature.com/articles/s41586-024-07566-y
- Loss of plasticity: https://www.nature.com/articles/s41586-024-07711-7
- LeCun AMI/JEPA framing: https://openreview.net/pdf?id=BZ5a1r-kVsf

## Residual Risk
I did not claim full CUDA/default-feature builds for every crate. Optional causal/graph agent checks that pull heavy RocksDB/embedding compilation were stopped earlier and are not part of this proof. The verified surface is the active retirement contract: retired local LLM/inner-loop code is absent from active CLI/MCP/tooling/instruction paths and the no-default CLI/MCP libraries compile.

## Related
- Decision: `memory/decisions/2026-05-09--single-outer-loop-mejepa-no-local-llm.md`
- FSV log: `/tmp/contextgraph-mejepa-retirement-fsv/run.log`
