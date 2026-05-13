---
namespace: reports
agent_id: codex
timestamp: 2026-05-12T16:45:30Z
status: active
tags: report, build, cargo, mold, cuda, fsv
---

# Session Report: 2026-05-12 — Builder Optimization FSV

## Task
Optimize the builder for the documented workstation specs while preserving fail-closed CUDA/toolchain behavior and proving the result with full state verification.

## Outcome
Complete. Commit `78498c04` tracks `.cargo/config.toml`, enables Cargo `jobs = 16`, uses Linux `mold`, keeps native CMake/NVCC fanout serial, restores release `codegen-units = 16`, and adds explicit `release-max` for single-codegen-unit final-throughput builds.

## Summary
The root cause was stale defensive serialization. The project had previously hit OOM from uncontrolled native/LLM build fanout, so local Cargo was pinned to one job. That fixed memory pressure but threw away the 9950X3D host's build capacity. The active dependency graph no longer includes the old `llama-cpp-sys` path that caused the nested `-j30` problem, so the durable fix is to parallelize Rust work while keeping native build scripts bounded.

Exa research confirmed the approach: Cargo's own build-performance guide recommends measuring target workflows and considering faster linkers such as `mold`; Cargo profile docs confirm higher codegen-units improve compile parallelism at a possible runtime-optimization cost; the Rust Performance Book recommends faster linkers where supported and keeping `codegen-units = 1` only when runtime speed matters more than compile time.

## Files Created or Modified in the Repo
- `.cargo/config.toml` — tracked project builder config: 16 Cargo jobs, mold linker, CUDA 13.2/RTX 5090 toolchain pins, bounded CMake fanout.
- `.gitignore` — unignored `.cargo/config.toml` while keeping other `.cargo` files local.
- `Cargo.toml` — release codegen-units restored to 16; added `release-max` profile with codegen-units 1.

## Files Written or Updated in ./memory/
- `discoveries/2026-05-12--builder-parallelism-root-cause-and-fsv.md` — root cause and FSV evidence.
- `reports/session-2026-05-12--builder-optimization-fsv.md` — this report.

## Decisions Made
- Normal `--release` builds optimize for local development speed with ThinLTO plus 16 codegen units.
- Final runtime-throughput builds should use `cargo build --profile release-max`.
- Native CMake/NVCC fanout remains serial by default to avoid multiplying Cargo job parallelism.

## Failures
- Edge-case invalid job count failed as expected: `jobs may not be 0`.
- Edge-case invalid linker failed as expected at `cc` with no fallback artifact replacement.
- Edge-case invalid `CUDA_ARCH=sm_999` failed as expected in `context-graph-cuda` build.rs before cubin replacement.

## Successes
- Full release build passed: `cargo build --release -p context-graph-cli -p context-graph-mcp -vv`.
- Full build evidence is under `/tmp/contextgraph-builder-fsv-20260512T163453Z`.
- Physical release artifacts were read back by `stat`, `file`, `sha256sum`, and `--help`.
- Warm no-op release build passed in 0.37s; post-edge recovery build passed in 0.61s.

## Tests / Build
- Tests added: none; this was build configuration.
- Tests passing: manual FSV happy path and 3 fail-closed edge cases passed.
- Build status: passing.

## Resume Here Next Session
- Start with `memory/discoveries/2026-05-12--builder-parallelism-root-cause-and-fsv.md` if changing builder config.
- Watch out for native build-script fanout. Do not raise `CMAKE_BUILD_PARALLEL_LEVEL` without a measured FSV run on the full CLI/MCP release graph.
