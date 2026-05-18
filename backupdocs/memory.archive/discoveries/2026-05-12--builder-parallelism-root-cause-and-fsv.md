---
namespace: discoveries
agent_id: codex
timestamp: 2026-05-12T16:45:30Z
status: active
tags: build, cargo, mold, cuda, fsv, wsl, rtx5090
---

# Builder Parallelism Root Cause And FSV

## What Happened
The local builder was underusing the workstation. `docs2/systemspecs.md` says the host is a Ryzen 9 9950X3D / RTX 5090 system, and WSL currently exposes 30 logical CPUs, but `.cargo/config.toml` pinned Cargo to `jobs = 1`. Release builds also pinned `codegen-units = 1`, which serialized the most expensive final crate codegen/link stage.

The serial Cargo pin was an over-correction from an older OOM incident where native/LLM CMake builds fanned out to host parallelism. That old `llama-cpp-sys` path is retired from the active dependency graph, but the serial Cargo setting remained.

## Root Fix
Commit `78498c04` makes `.cargo/config.toml` tracked project config and sets:

- `jobs = 16` for physical-core-scale Rust fanout.
- Linux rustflags `-C link-arg=-fuse-ld=mold`, with no fallback.
- `CMAKE_BUILD_PARALLEL_LEVEL = 1` so native CMake/NVCC work cannot multiply Cargo's job count into the historical OOM shape.
- Release `codegen-units = 16` for normal development release builds.
- `profile.release-max` with `codegen-units = 1` for explicit final-throughput builds.

## FSV Evidence
Source of truth: `.cargo/config.toml`, root `Cargo.toml` profiles, and physical artifacts under `target/release/`.

Evidence directory: `/tmp/contextgraph-builder-fsv-20260512T163453Z`.

Observed happy path:

- Full invalidating release build: `cargo build --release -p context-graph-cli -p context-graph-mcp -vv` passed in 5m13s.
- The verbose log contains `NUM_JOBS=16`, `CMAKE_BUILD_PARALLEL_LEVEL=1`, `-C codegen-units=16`, and `-C link-arg=-fuse-ld=mold` on the final CLI/MCP rustc invocations.
- `target/release/context-graph-cli` and `target/release/context-graph-mcp` both exist, are ELF stripped executables, and both respond to `--help`.
- Warm rebuild after artifacts were current passed in 0.37s; post-edge recovery passed in 0.61s.

Edge cases:

- `CARGO_BUILD_JOBS=0 cargo check -p context-graph-paths --quiet` failed with `jobs may not be 0`, and release binary mtimes were unchanged.
- Invalid linker flag via `cargo rustc --release -p context-graph-cli --bin context-graph-cli -- -C link-arg=-fuse-ld=/tmp/contextgraph-missing-mold-linker` failed at the linker, and release binary mtimes were unchanged.
- `CUDA_ARCH=sm_999 cargo build --release -p context-graph-cuda --quiet` failed in `build.rs` with the RTX 5090 `sm_120a` requirement, and the cubin hash/mtime were unchanged.

## Why It Matters
Future builder work should keep Rust parallelism and native fanout separate. Increasing both layers together can recreate the old OOM. For this machine, Cargo job fanout is the right place to spend CPU; native CUDA/CMake fanout should remain bounded unless measured under FSV.

## Related
- See also: `/home/cabdru/contextgraph/docs2/systemspecs.md`
- See also: `memory/discoveries/session-2026-05-09--dynamicjepa-ts-csharp-build-root-causes.md`
