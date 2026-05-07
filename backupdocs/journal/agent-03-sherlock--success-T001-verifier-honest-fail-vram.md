---
namespace: journal
agent_id: agent-03-sherlock
timestamp: 2026-04-29T04:14:00Z
status: active
severity: info
tags: exp2, t-001, verify-environment, fail-loud, c16, runtime-verified
---

# T-001 verify_environment.py honestly fails on insufficient VRAM (Constitution C16 satisfied)

## Command
```
python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml --skip-cargo
```

## Live state at run
- nvidia-smi: RTX 5090, 32607 MiB total, 17241 MiB free (under 30000 MiB threshold).
- Daemon: 3 context-graph-mcp processes alive (PIDs 4402, 66402, 67343); 67343 listening on 3100.

## Stderr/stdout (verbatim)
```
2026-04-28 23:12:58,911 ERROR Environment verification failed: ["cuda_gpu_rtx5090_30gb_free failed: [{'name': 'NVIDIA GeForce RTX 5090', 'total_mib': 32607, 'free_mib': 17241}]"]
EXP-2 environment verification failed: cuda_gpu_rtx5090_30gb_free failed: [{'name': 'NVIDIA GeForce RTX 5090', 'total_mib': 32607, 'free_mib': 17241}]
```

## Exit code
**1** (verified directly without tee/pipe). Earlier my piped capture via `tee` reported 0 because PIPESTATUS[0] vs $? — not a code defect, just my tooling.

## Source-of-truth verification
- `/home/cabdru/contextgraph/artefacts/exp2/verification_log.json` updated on this run; cuda check shows ok=false with free_mib=17237; weight checks all ok=true; weight_hashes block populated for 10 weight files (sha256).
- `/home/cabdru/contextgraph/artefacts/exp2/audit_chain_environment.json` — single-event chain (`environment_verified`), one artefact (`verification_log`), chain_sha256=6fc5bd0b8b…, code_commit_sha=2679ea1bf4ad…, hyperparameters_sha256=32e6ad7dbd…, finalized.
- `/home/cabdru/contextgraph/artefacts/exp2/run.log.jsonl` records 9 historical runs (mixed ERROR fail and INFO pass), confirming the script writes JSONL audit logs deterministically.

## Verdict
- T-001 enforces C16 fail-loud correctly.
- The verifier IS the only place AuditChain is wired in production (confirms agent-01 B-2 + agent-02 refinement: 1 stub instantiation, no master chain across phases).
- The failure here is the EXPECTED state — GPU is too occupied for the production extract step. NOT a defect of the verifier; it is the verifier doing its job.

## Discrepancies vs prior reports
- Agent-02 said "I will skip the cargo build". The current verification_log.json has no `cargo_build_release_context_graph_mcp` entry, consistent with `--skip-cargo`. Confirms agent-02's command worked as advertised.

## Confidence: HIGH (independently reproduced exit-1 behaviour, verified all artefact mutations).
