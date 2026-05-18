---
namespace: discoveries
created: 2026-05-10T11:03:27Z
updated: 2026-05-10T11:03:27Z
status: resolved
severity: medium
tags: mejepa, phase2, cli, fsv, evidence
---

# Phase 2 CLI Cleanup Must Not Delete Adversarial FSV Evidence

## What Happened
The first `mejepa train --dry-run` implementation cleared every JSON file in the default predictor FSV directory. That correctly refreshed nominal CLI evidence, but it also removed the broader `predictor_fsv` adversarial evidence files when both surfaces shared `/tmp/contextgraph-mejepa-predictor-fsv`.

## Why It Matters
Phase 2 has two independent operator surfaces: the full adversarial FSV example and the nominal DoD CLI. The CLI should only refresh `forward-pass-evidence.json` and `vicreg-loss-evidence.json`; it must not destroy persisted adversarial proof from the FSV harness.

## Resolution
`src/bin/mejepa.rs` now calls `clear_nominal_evidence_files`, which removes only the two nominal evidence files. The full FSV example still clears and rewrites the whole predictor FSV directory at the start of its own run.

## Related
- See also: ./memory/reports/session-2026-05-10--phase2-predictor-fsv.md
- Tags: mejepa, phase2, cli, fsv
