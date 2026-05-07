---
namespace: journal
agent_id: agent-01-static-audit
timestamp: 2026-04-28T23:07:00Z
status: active
severity: medium
tags: exp2, effective-rank, spectrum, eigvalsh, math
---

# Issue: n_eff_spectrum_entropy uses np.linalg.eigvalsh on a normalized M but Schur uses np.linalg.svd on un-normalized M

## Where
`scripts/exp2/effective_rank.py:36-49`

## Observation 1: spectrum_entropy normalizes Frobenius then calls eigvalsh
```python
def n_eff_spectrum_entropy(mi_matrix: np.ndarray) -> tuple[float, np.ndarray]:
    norm = np.linalg.norm(mi_matrix, ord="fro")
    eigenvalues = np.linalg.eigvalsh(mi_matrix / norm)
    weights = np.abs(eigenvalues)
    weights = weights / np.sum(weights)
    entropy = -float(np.sum(weights * np.log(weights + 1e-15)))
    return float(np.exp(entropy)), eigenvalues
```
`eigvalsh` requires a Hermitian matrix. The MI matrix is filled symmetrically by `mi_matrix_from_rows` (line 69-70: `m[i, j] = m[j, i] = value`) AND `main()` sets `mi[i, i] = entropy[i]` AT line 132-133 BEFORE calling `n_eff_spectrum_entropy`. So at call time the diagonal is the entropy, the off-diagonal is symmetric MI. Hermitian. Fine.

## Observation 2: schur_complement uses svd on the same matrix
```python
def n_eff_schur_complement(mi_matrix: np.ndarray, threshold: float = 0.05) -> int:
    sv = np.linalg.svd(mi_matrix, compute_uv=False)
    return int(np.sum(sv > threshold * sv[0]))
```
`svd` doesn't require Hermitian; for a real symmetric PSD matrix, sv = abs(eigenvalues). Acceptable.

## Concern: returned eigenvalues are NORMALIZED-MATRIX eigenvalues (line 40), but the summary ships `eigenvalues.tolist()` (line 150)
The variable name `eigenvalues` is from `np.linalg.eigvalsh(mi_matrix / norm)` — they are eigenvalues of the *normalized* MI matrix, scaled by `1/norm`. The spec (`08_output_deliverables.md` §5 — Spectrum Plot) and `05_technical_spec.md` say:
> X-axis: rank index (1 to 14). Y-axis: eigenvalue magnitude (log scale).

This is ambiguous. Consumer of the eigenvalues may expect raw, not normalized. Should be documented or always reported in raw form.

## Concern 3: mi_matrix is NOT positive semi-definite in general
Mutual information has the property `I(X;Y) ≥ 0`, but the matrix `M[i,j] = I(E_i; E_j)` for `i≠j` and `M[i,i] = H(E_i)` is **not** guaranteed to be PSD. It's a Gram-like matrix but not built from inner products. So `eigvalsh` could return negative eigenvalues.

The implementation handles this with `weights = np.abs(eigenvalues)` (line 41) — good.

But: the spec example in `08_output_deliverables.md` §1 says `n_eff_spectrum_entropy: 11.8` for a 14-panel. With `np.exp(H(uniform_14))=14`, getting 11.8 is plausible. But if some eigvalues are negative, the spectrum interpretation is off-spec.

## Why it matters
- Numerically the implementation is robust. Theoretically, the methodology in `03_methodology.md` §7b doesn't explicitly handle negative eigenvalues. It says "Compute eigenvalues; Normalize; Spectral entropy." It implicitly assumes non-negative.
- Document or add a check.

## Recommended fix
Either:
1. Flag negative eigenvalues in audit-chain events (warn, don't fail) so the methodology gap is visible.
2. Use `np.linalg.svd` for spectral entropy too — singular values are non-negative.

## Severity
MEDIUM. This won't crash; it produces a number. But that number's interpretation depends on a methodological choice (`abs` of eigenvalues) that is not in the spec.
