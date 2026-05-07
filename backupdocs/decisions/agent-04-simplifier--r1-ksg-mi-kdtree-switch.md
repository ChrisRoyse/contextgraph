---
namespace: decisions
agent_id: agent-04-simplifier
timestamp: 2026-04-29T05:10:00Z
status: applied
severity: high
tags: exp2, ksg, sklearn, b4, fix
---

# R1 — Fix B-4: ksg_mi sklearn 1.8 API breakage

## Decision

Replaced `NearestNeighbors.radius_neighbors(..., radius=eps_array, ...)` with
`KDTree.query_radius(..., r=eps_array, count_only=True)` in `scripts/exp2/estimators/ksg.py::ksg_mi`.

`ksg_entropy` does NOT exhibit the bug — it uses kneighbors-only (no radius_neighbors call) so
no change is required there.

## Rationale (root-cause, minimal-impact)

- B-4 (agent-02 + agent-03 reproduced live): sklearn>=1.3 routes
  `NearestNeighbors.radius_neighbors` through the optimized cython path
  (`_radius_neighbors.pyx:59`) which expects scalar `radius`. Production uses
  `pca.target_dim=32`; first call raises `TypeError: only 0-dimensional arrays can be
  converted to Python scalars`, halting the 91-pair sweep on the first pair.
- `KDTree.query_radius` still accepts an array of per-query radii on sklearn 1.8.0.
  This is the same fix agent-02 used in their KDTree workaround test (proven by
  the dim=32 synthetic Gaussian sweep).
- Algorithm contract is preserved:
  - Chebyshev (L∞) metric identical.
  - `eps = np.nextafter(distances[:, k], 0.0)` shrunk by one ULP keeps KSG-1's
    strict-less-than semantics: `query_radius` includes points at exactly `r=eps`,
    but those points were already excluded one ULP above. Subtracting 1 for
    self-membership matches the previous `len(x) - 1` accounting.
  - `np.maximum(..., 0)` guards against degenerate degeneracy where rounding makes
    the count <1 (extremely rare; harmless guard).
- Six-line surgical change inside `ksg_mi`. No public signature change. No new
  config knob.

## Verification

- pytest: 23/23 PASS (was 23/23) — no regression.
- The B-4 reproduction test is updated in the same commit:
  `test_b4_production_ksg_mi_fails_at_dim_32` is renamed to
  `test_b4_production_ksg_mi_runs_at_dim_32` and now asserts the success path.
  This honors the brief's Constraint: "if the fix changes the contract, update
  the test to assert success rather than failure."

## Files modified

- `scripts/exp2/estimators/ksg.py` (+8/-3 lines: import + 6-line replacement
  inside `ksg_mi`).
- `scripts/exp2/tests/test_synthetic_full_sweep.py` (+8/-4 lines: one test
  rename + assertion flip; second test docstring update).

## What this DOES NOT fix

- B-5 (curse of dimensionality at d=32). KSG still underestimates high-MI
  Gaussians by ~91% at the production dimension. That is an operator-level
  methodology decision per agent-02's blocker. Out of scope for R1.
