---
namespace: context
agent_id: agent-00-coordinator
timestamp: 2026-04-14T16:30:00Z
status: active
tags: constraints, tech-stack, non-negotiables
---

# Global Constraints

## Tech Stack (Locked)
- Rust 2021 edition, MSRV 1.75
- Workspace: 10 crates under `crates/`
- RocksDB for all persistence — NO SQLite, NO other KV
- Bincode for dense data, JSON for provenance/optional fields
- HNSW for dense embeddings (hnsw_rs 0.3.3)
- CUDA 13.2 via `CUDARC_CUDA_VERSION=13000` env override
- Parquet via `parquet = "58"` for training corpus export

## Embedder Architecture (13 Total)
- E1 (1024D semantic), E2-E4 (512D temporal), E5 (768D causal asymmetric dual)
- E6 (30K sparse keyword), E7 (1536D code), E8 (1024D graph dual)
- E9 (1024D HDC), E10 (768D paraphrase dual), E11 (768D entity — DISABLED)
- E12 (128D/token ColBERT MaxSim), E13 (30K SPLADE inverted)

## Behavioral Rules (Non-Negotiable)
- NO backwards compatibility — if it breaks, fail fast with logs
- NO mock data in tests — real code paths with known-input synthetic fixtures
- NO tests that pass while the system is broken
- NO workarounds / fallbacks — error out with diagnostics
- NO writing to root folder — use `/src`, `/tests`, `/docs`, `/tasks`, `/memory`
- Files < 500 lines
- Every MCP tool must be in `tool_dispatch!` macro in `handlers/tools/dispatch.rs`

## Math Invariants (Per Plan)
- Cross-correlations count = C(13,2) = 78
- GroupAlignment count = 6 (Factual, Temporal, Causal, Relational, Qualitative, Implementation)
- TopicProfile length = 13 (one per embedder space)
- SRC-3 cosine normalization: `(raw + 1) / 2` → [0, 1]

## File Locations (Today's New Artifacts)
Per `tasks/final_fsv_report.md` §"Files landed this session":
- `crates/context-graph-core/src/{training,constellation,contrastive,teleological/tucker}.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/{export,constellation,contrastive}.rs`
- `crates/context-graph-storage/tests/{training_records,constellation,contrastive}_integration.rs`
- `crates/context-graph-mcp/src/tools/definitions/{training,constellation,contrastive}.rs`
- `crates/context-graph-mcp/src/handlers/tools/{training,constellation,contrastive}_tools.rs`
- `crates/context-graph-cli/src/{lib,commands/export_training}.rs`
