---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:18:00Z
status: active
severity: low
tags: exp2, verify-environment, vram, fail-loud, success
---

# Success: verify_environment.py rigorously enforces 30 GiB free VRAM and writes its own audit chain

## What's good
- Hard requirement: GPU named "RTX 5090" AND `total_mib >= 30000` AND `free_mib >= 30000`. Per `tasks/exp2/lessons.md` (2026-04-28), this exact rule was added after the false-pass incident.
- Parses `nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv,noheader` directly. (Lines 56-72.)
- All required Python packages enumerated. Failure listed by name. (Lines 19, 53-54.)
- Cargo build is mandatory unless `--skip-cargo` flag is passed. (Lines 77-85.)
- Each weight file's existence is checked AND its sha256 is computed. (Lines 87-94.)
- Verification log written to `artefacts/exp2/verification_log.json` per spec.
- AuditChain INSTANTIATED here — first known caller. (lines 98-101.)
- Audit chain finalized BEFORE the failure raise (line 101 → line 104), so even on failure the chain is persisted. Good.

## Note: the audit chain ONLY captures the environment phase
`audit_chain_environment.json` is created here but is NOT linked into the master `audit_chain.json` (which doesn't exist; see `agent-01-static-audit--no-audit-chain-built-anywhere.md`). So the environment chain stands alone — useful for T-001 evidence, but no chain links it to T-007 or any later phase.

## Note: weight file list (lines 20-31) only enumerates 10 paths
The 14-embedder panel includes:
- E1 semantic, E2/E3/E4 (sin-PE, no weights file — produced by formula not model)
- E5 causal, E6 sparse, E7 code-1536, E8 graph, E9 HDC (also no weights — projected hyperdimensional vectors)
- E10 multimodal/paraphrase, E11 entity, E12 late-interaction (ColBERT), E13 splade-v3, E14 bge-m3-dense

The 10 enumerated paths cover models that have on-disk weights. E2-E4 and E9 are computed/derived, no model file. So 10 is correct.

## Compliance
- C1: weight hashes recorded, fits "embedder frozen at pinned weight hashes". Good.
- C7: audit-chain partial (environment-only) — but exists at all, which is more than other modules.

## Recommended improvement
- Have the audit-chain output of this phase be ingested by the master pipeline chain (when the master chain is wired per the missing-audit-chain blocker).
