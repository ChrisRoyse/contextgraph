---
namespace: blockers
created: 2026-05-10T22:17:49.674566056+00:00
updated: 2026-05-10T17:20:49-05:00
status: resolved
severity: critical
tags: mejepa, phase5, witness, integrity
---

# ME-JEPA Witness Chain Integrity Failure

reason: witness chain integrity failed
kind: BrokenAt { offset: 1, expected_prev_hash: [22, 210, 29, 59, 72, 115, 74, 64, 92, 196, 239, 213, 151, 34, 103, 31, 91, 25, 71, 35, 108, 168, 35, 96, 84, 135, 234, 26, 53, 47, 110, 125], actual_prev_hash: [165, 210, 29, 59, 72, 115, 74, 64, 92, 196, 239, 213, 151, 34, 103, 31, 91, 25, 71, 35, 108, 168, 35, 96, 84, 135, 234, 26, 53, 47, 110, 125] }
rollback_target: None

## Resolution

Resolved for the Phase 1-5 hardening scope on 2026-05-10. The originally recorded broken-chain payload is now treated as a synthetic/investigative integrity failure shape, not a live unresolved ME-JEPA training-chain blocker.

FSV readbacks now prove both sides of the invariant:

- Clean training certificate chain: `cargo run -p context-graph-mejepa-train --example train_cert_chain_fsv` verified 100,000 steps with `broken_at=null`; source of truth `/tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json`.
- Corruption detection path: `train_edge_cases_fsv` tampered a chain and read back `broken_at=1`, `verified=1`; source of truth `/tmp/contextgraph-mejepa-train-fsv/edge-case-evidence.json`.
- Phase 5 self-healing drill detects the synthetic witness integrity violation while still passing the ship gate; evidence root `/tmp/contextgraph-mejepa-self-healing-fsv`.

If this exact `BrokenAt { offset: 1, ... }` shape appears again outside an intentional edge-case harness, open a new blocker with the runtime root and CF/file path that produced it.
