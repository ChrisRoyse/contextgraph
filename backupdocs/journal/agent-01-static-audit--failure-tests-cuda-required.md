---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:30:00Z
status: active
severity: medium
tags: exp2, tests, cuda, infonce, ci-blocking
---

# Defect: InfoNCE.fit() requires CUDA in __init__; tests cannot exercise it on CPU-only

## Where
`scripts/exp2/estimators/infonce.py:53-56`

```python
requested = device or hyperparams.get("device", "cuda")
if requested == "cuda" and not torch.cuda.is_available():
    raise Exp2Error("InfoNCE requested CUDA but torch.cuda.is_available() is false")
```

## Constraint
- `tasks/exp2/todo.md` reports T-011 "completed" but no test for InfoNCE exists.
- exp2.yml has `infonce.device: cuda` (line 35).
- Tests that imported InfoNCE would fail at construction time without CUDA.

## Why it's a defect
- The methodology says "InfoNCE on RTX 5090" but the unit-test layer should still cover the algorithm's correctness on CPU. Without that, the implementation can't be unit-tested in any CI environment without GPU.
- If a contributor in a CPU-only environment changes InfoNCE, they cannot verify their change.
- T-011's DoD calls for synthetic ground-truth tests; these run fine on CPU at small dims (32-D, n=400).

## Recommended fix
- The fail-loud is correct ONLY for production runs. For tests, allow `device="cpu"` and have the InfoNCE fit work end-to-end on CPU.
- Add `pytest.skip_if_no_cuda` decorator only on tests explicitly requiring CUDA.
- Add a test that constructs `InfoNCEEstimator(hp, device="cpu")` and runs a small fit.

## Severity
MEDIUM. Coverage gap — no InfoNCE test means correctness depends on the production run not failing weirdly.
