# Session Report: WSL VRAM Telemetry Hardening

Date: 2026-05-12T22:10:00Z

## Scope

Closed GitHub issue #10: `[RISK] Harden VRAM telemetry gates under WSL GPU limitations`.

## Root Cause

The embedder VRAM budget gate correctly used CUDA driver memory readback, but the `nvidia-smi` side channel was hard-wired, called twice, and lacked explicit WSL sentinel tests. NVIDIA documents `nvidia-smi` as limited under WSL 2, so absent or partial `nvidia-smi` output must be diagnostic only and must not become the authoritative budget source.

## Changes

- Added injectable `nvidia_smi_total_mb_from_command`.
- Added `parse_nvidia_smi_memory_total_mb` and `nvidia_smi_status`.
- Recognized WSL unsupported rows including `N/A`, `[Not Supported]`, `Not-Supported`, and `unsupported`.
- Stopped executing `nvidia-smi` twice per budget report.
- Added `vram_wsl_telemetry_fsv`.
- Updated `docs/futurebuild/02_CURRENT_STATE_HONEST.md`.

## Full State Verification

Primary source of truth:

- `/mnt/d/contextgraph/fsv/contextgraph-mejepa-vram-wsl-fsv/vram-wsl-telemetry-evidence.json`
- `/home/cabdru/contextgraph/docs2/systemspecs.md`

Physical readback:

- evidence size: 4,098 bytes
- evidence mode: `0600`
- evidence sha256: `778c1c8992a9be44808770fa476e85a07fba2a288295a341ccab5274497c1f66`

Real CUDA readback:

- GPU: `NVIDIA GeForce RTX 5090`
- compute capability: `12.0`
- telemetry source: `cuda_driver_cuMemGetInfo_v2`
- total bytes: `34190458880`
- free bytes: `32429309952`
- `nvidia_smi_status`: `diagnostic_available_total_mb=32607`

Edge cases:

- Missing `nvidia-smi` command: diagnostic unavailable, non-authoritative.
- WSL unsupported `N/A`: diagnostic unavailable, non-authoritative.
- Malformed `32607 MiB`: parse failure, diagnostic unavailable.
- Valid `32607`: diagnostic available.

## Commands

- `cargo test -p context-graph-mejepa-embedders vram -- --nocapture`
- `cargo test -p context-graph-mejepa-embedders --examples --no-run`
- `cargo run -p context-graph-mejepa-embedders --example vram_wsl_telemetry_fsv`
- `cargo test -p context-graph-mejepa-embedders -- --nocapture`

## Web Research

Used Exa MCP for:

- NVIDIA CUDA on WSL User Guide: `nvidia-smi` has limited feature coverage under WSL 2.
- NVIDIA CUDA 13.2 compute capability appendix.
- NVIDIA CUDA GPU compute capability table confirming RTX 5090 as compute capability 12.0.
