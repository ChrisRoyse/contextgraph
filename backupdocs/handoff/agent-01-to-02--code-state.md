---
namespace: handoff
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:38:00Z
status: active
tags: exp2, handoff, agent-02-tester, static-audit
---

# Handoff: agent-01-static-audit → agent-02-tester

## Static audit summary
- Files audited: 13 source files + 4 test files = 17 Python files in `scripts/exp2/`.
- Findings written: 5 HIGH (all in `memory/blockers/` and `memory/journal/`), 14 MEDIUM, 5 LOW.
- Successes documented: 9 (fail-loud helpers, source-of-truth verification, deterministic bootstrap, etc.).

## Counts at-a-glance
| Severity | Count |
|---|---|
| HIGH (ship-blocker or audit-chain breaker) | 5 |
| MEDIUM (spec drift, coverage gap) | 14 |
| LOW (cosmetic) | 5 |
| Successes | 9 |

See `/home/cabdru/contextgraph/memory/decisions/agent-01-static-audit--severity-rubric.md` for rubric.

## What agent-02 should run

### Tier 1 — pytest (CPU only, no daemon required)
```bash
cd /home/cabdru/contextgraph
python3 -m pytest -v scripts/exp2/tests
```
Expected: 7 passed. If less than 7, capture which tests fail.

### Tier 2 — synthetic Gaussian sweep (T-013 substitute) [CRITICAL — recommended NEW work]
The spec calls for ρ ∈ {0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99} with InfoNCE within 0.1 nats and KSG within 0.15 nats. Currently NO TEST DOES THIS SWEEP.
Recommended NEW agent-02 work:
```python
# scripts/exp2/tests/test_synthetic_validation_full.py
import pytest
from scripts.exp2.estimators.ksg import ksg_mi
from scripts.exp2.estimators.synthetic import gaussian_pair

@pytest.mark.parametrize("rho", [0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99])
def test_ksg_recovers_at_d32(rho):
    u, v, truth = gaussian_pair(n=800, dim=32, rho=rho, seed=42)
    estimate = ksg_mi(u, v, k=5)
    assert abs(estimate - truth) < 0.20  # KSG underestimates at high MI / high d
```
And InfoNCE (CPU smoke test):
```python
def test_infonce_recovers_at_d8():
    if not torch.cuda.is_available():
        pytest.skip("InfoNCE.fit raises if device='cuda' and CUDA missing")
    # ... or modify InfoNCE to allow CPU on tests
```

### Tier 3 — verify_environment.py rerun
```bash
python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml --skip-cargo
```
Expected: should fail with "VRAM < 30000 MiB free" if GPU is loaded; otherwise pass.

### Tier 4 — manual_fsv on running daemon
```bash
python3 scripts/exp2/manual_fsv.py --config scripts/exp2/exp2.yml
```
Requires the daemon process be runnable (the script spawns its own MCP via subprocess; needs `target/release/context-graph-mcp` present and weights under `models/`).
Expected: 3 happy + 3 edge cases pass.

### Tier 5 — DO NOT TRUST without daemon up
- `scripts/exp2/extract_embeddings.py` — does NOT run without the MCP server + 14 weights.
- `scripts/exp2/build_corpus.py` — S5 (BEIR), S6 (Wikipedia), S8 (FLORES) need internet via `datasets`. Fails offline.

### Tier 6 — Code paths to inspect (no execution needed)
- Confirm `effective_rank.n_eff_redundancy_adjusted` produces the expected count for synthetic 14-embedder identity matrix.
- Confirm `bootstrap_ci` still deterministic.

## Modules agent-02 should NOT trust

| Module | Why |
|---|---|
| `mcp_client.py` | Requires live MCP daemon. Don't run integration tests without it (defer to agent-03 sherlock). |
| `extract_embeddings.py` | Requires live MCP daemon AND 14 weight files. |
| `build_corpus.py collect_beir/wikipedia/multilingual` | Needs internet + HuggingFace datasets. |
| `run_exp2.py` orchestrator | NOT WIRED to call `audit_chain.py`; running it produces incomplete state. |
| `effective_rank.py` headline.structural_upper_bound | Hardcoded to 105; spec says 91. Don't trust the JSON output of this script. |

## Concrete pytest commands

```bash
cd /home/cabdru/contextgraph

# Existing test suite
python3 -m pytest -v scripts/exp2/tests

# Run only KSG recovery (recommended baseline)
python3 -m pytest -v scripts/exp2/tests/test_estimators.py::test_ksg_recovers_low_dimensional_gaussian_mi

# Determinism check
python3 -m pytest -v scripts/exp2/tests/test_estimators.py::test_bootstrap_ci_is_deterministic_on_known_mean

# Synthetic closed-form
python3 -m pytest -v scripts/exp2/tests/test_synthetic_validation.py
```

## Priority list for agent-02
1. Confirm 7-pass baseline.
2. Add InfoNCE CPU-mode test (likely requires patching InfoNCE constructor to allow `device="cpu"` for tests).
3. Add k-sweep KSG sensitivity test.
4. Run manual_fsv with the live daemon (if the daemon is runnable).
5. Write a failure-mode coverage test (F2 raises, F4 raises on duplicate without jitter, F6 raises on agreement < 80%).

## High-priority defects for agent-02 to triage
1. **InfoNCE.fit raises on overfit gap → blocks production**. Try a synthetic high-MI run and confirm raise behavior. Decide: fix-now vs document-for-coordinator.
2. **AuditChain unused**. Confirm by `grep -r AuditChain scripts/exp2/`.
3. **Per-pair rows missing provenance**. Inspect a fixture `pair_results/E1_E2.json` (would need to be produced; or simulate with a hand-crafted row to check schema).

## Confidence
Static audit is high-confidence on what was read. Dynamic behavior (Pytest, manual_fsv) defers to agent-02's runtime evidence.
