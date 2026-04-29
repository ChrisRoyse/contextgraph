"""T-013 full synthetic Gaussian rho-sweep validation.

Spec: docs/exp2/03_methodology.md §12, docs/exp2/07_validation_harness.md §3a.

For each rho in {0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99}:
  Generate u ~ N(0, I_d), v = rho*u + sqrt(1-rho^2)*eps, dim=32, n=800.
  Closed-form MI = -0.5 * d * log(1 - rho^2) nats.
  KSG within 0.20 nats for rho <= 0.7 (spec §3a tolerance is 0.15).

This test writes /home/cabdru/contextgraph/artefacts/exp2/synthetic_validation_report.json
summarising truth, ksg estimate, abs error, pass for each rho.

Also includes:
- k-sweep stability (M6): for rho=0.5, KSG with k in {3, 5, 10}.
- Bootstrap determinism (M7): two identical bootstrap_ci calls produce identical output.
- Failure-mode coverage (M8): F4 KSG NaN, F2 InfoNCE overfit -> raise.

This test uses the production KSG implementation. If the production estimator
cannot recover the low-rho ground truth within tolerance, EXP-2 must stop.
"""

from __future__ import annotations

import json
import math
from pathlib import Path

import numpy as np
import pytest
from scripts.exp2.estimators.bootstrap import bootstrap_ci
from scripts.exp2.estimators.ksg import ksg_mi
from scripts.exp2.estimators.synthetic import gaussian_pair
from scripts.exp2.common import Exp2Error


REPO_ROOT = Path(__file__).resolve().parents[3]
ARTEFACTS_DIR = REPO_ROOT / "artefacts" / "exp2"
REPORT_PATH = ARTEFACTS_DIR / "synthetic_validation_report.json"


RHO_SWEEP = [0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 0.99]
TOL_LOW_RHO = 0.15  # for rho <= 0.7 per docs/exp2/07_validation_harness.md
DIM = 32
N = 800


def _truth_mi_nats(rho: float, dim: int) -> float:
    """Closed-form Gaussian MI in nats. Returns 0 when rho^2 hits 1."""
    rho2 = rho * rho
    if rho2 >= 1.0:
        rho2 = 1.0 - 1e-12
    return -0.5 * dim * math.log(1.0 - rho2)


# ---------------------------------------------------------------------------
# B-4 verification — confirm production ksg_mi blows up at dim>=20.
# ---------------------------------------------------------------------------


def test_b4_production_ksg_mi_runs_at_dim_32():
    """B-4 fix verification: production ksg_mi must complete at dim=32 on sklearn>=1.3.

    Before the fix this raised TypeError from sklearn's _radius_neighbors.pyx because
    NearestNeighbors.radius_neighbors no longer accepts array radii at high dim.
    The fix in ksg.py switches to KDTree.query_radius (which still accepts array radii).
    """
    u, v, _ = gaussian_pair(n=200, dim=32, rho=0.5, seed=0)
    estimate = ksg_mi(u, v, k=5)
    assert math.isfinite(estimate)
    assert estimate >= 0.0


def test_b4_production_ksg_mi_works_at_dim_1():
    """Confirms production ksg_mi still works at low dim after the B-4 fix."""
    u, v, truth = gaussian_pair(n=400, dim=1, rho=0.5, seed=0)
    estimate = ksg_mi(u, v, k=5)
    assert math.isfinite(estimate)
    # Loose check; just confirm finite execution.
    assert estimate >= 0.0


# ---------------------------------------------------------------------------
# M2 — Synthetic Gaussian sweep using KDTree workaround
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def sweep_results():
    """Run full sweep once, expose results to all parametrized tests."""
    results = {}
    for rho in RHO_SWEEP:
        u, v, truth_from_helper = gaussian_pair(n=N, dim=DIM, rho=rho, seed=42)
        truth = _truth_mi_nats(rho, DIM)
        assert math.isclose(truth_from_helper, truth, rel_tol=1e-10, abs_tol=1e-10), (
            f"gaussian_pair truth disagreement at rho={rho}: helper={truth_from_helper} vs closed={truth}"
        )
        ksg = ksg_mi(u, v, k=5)
        err = abs(ksg - truth)
        if rho <= 0.7:
            passed = err <= TOL_LOW_RHO
        else:
            passed = True
        results[f"rho_{rho:.2f}"] = {
            "rho": rho,
            "truth_nats": truth,
            "ksg_nats": float(ksg),
            "abs_error_nats": float(err),
            "tolerance_nats": TOL_LOW_RHO if rho <= 0.7 else None,
            "pass": bool(passed),
        }
    ARTEFACTS_DIR.mkdir(parents=True, exist_ok=True)
    overall_pass = all(r["pass"] for r in results.values())
    payload = {
        "config": {
            "n": N,
            "dim": DIM,
            "rho_sweep": RHO_SWEEP,
            "k_ksg": 5,
            "seed": 42,
            "tolerance_nats_rho_le_0.7": TOL_LOW_RHO,
            "ksg_implementation": "production: scripts.exp2.estimators.ksg.ksg_mi",
        },
        "results": results,
        "overall_pass": bool(overall_pass),
    }
    REPORT_PATH.write_text(json.dumps(payload, indent=2, sort_keys=True))
    return results


@pytest.mark.parametrize("rho", RHO_SWEEP)
def test_ksg_recovers_gaussian_at_d32_or_fails_loudly(sweep_results, rho):
    """Production KSG must satisfy the pre-production synthetic acceptance gate."""
    key = f"rho_{rho:.2f}"
    row = sweep_results[key]
    assert math.isfinite(row["ksg_nats"])
    assert row["ksg_nats"] >= 0.0
    if rho <= 0.7:
        assert row["pass"], (
            f"KSG synthetic gate failed at rho={rho}: "
            f"truth={row['truth_nats']:.6f}, estimate={row['ksg_nats']:.6f}, "
            f"abs_error={row['abs_error_nats']:.6f}, tolerance={TOL_LOW_RHO}"
        )


def test_synthetic_report_written(sweep_results):
    """Report file exists and has all 7 rho rows."""
    assert REPORT_PATH.exists()
    payload = json.loads(REPORT_PATH.read_text())
    assert set(payload["results"].keys()) == {f"rho_{r:.2f}" for r in RHO_SWEEP}
    assert "overall_pass" in payload


# ---------------------------------------------------------------------------
# M6: KSG k-sweep sensitivity (spec §5b)
# ---------------------------------------------------------------------------


def test_ksg_k_sweep_stability_at_rho_0_5():
    """Spec: 'If pilot disagreement > 0.1 nats across k, methodology revisits.'"""
    rho = 0.5
    u, v, truth = gaussian_pair(n=N, dim=DIM, rho=rho, seed=42)
    estimates = {k: ksg_mi(u, v, k=k) for k in (3, 5, 10)}
    spread = max(estimates.values()) - min(estimates.values())
    for k, val in estimates.items():
        assert math.isfinite(val)
    out = ARTEFACTS_DIR / "ksg_k_sweep_rho_0.5.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(
        json.dumps(
            {
                "rho": rho,
                "n": N,
                "dim": DIM,
                "truth_nats": truth,
                "estimates": {str(k): float(v) for k, v in estimates.items()},
                "spread_nats": float(spread),
                "spec_threshold_nats": 0.1,
                "stable": bool(spread <= 0.1),
                "ksg_implementation": "production: scripts.exp2.estimators.ksg.ksg_mi",
            },
            indent=2,
            sort_keys=True,
        )
    )
    assert spread <= 0.1, f"KSG k-sweep instability {spread:.6f} exceeds 0.1 nats"


# ---------------------------------------------------------------------------
# M7: bootstrap determinism (Constitution C6)
# ---------------------------------------------------------------------------


def test_bootstrap_ci_is_deterministic_re_check():
    """Same seed + same inputs => identical CI + bootstrap sample vector."""
    rng = np.random.default_rng(7)
    sample = rng.normal(size=200)

    def point(idx):
        return float(np.mean(sample[idx]))

    low1, high1, vec1 = bootstrap_ci(point, n_items=sample.size, n_resamples=2000, seed=42)
    low2, high2, vec2 = bootstrap_ci(point, n_items=sample.size, n_resamples=2000, seed=42)
    assert low1 == low2
    assert high1 == high2
    assert np.array_equal(vec1, vec2)


# ---------------------------------------------------------------------------
# M8: F4 KSG NaN/Inf detection (must raise)
# ---------------------------------------------------------------------------


def test_ksg_raises_on_non_finite_input_dim_1():
    """At dim=1 the production path is reachable; verify NaN raises."""
    rng = np.random.default_rng(0)
    u = rng.normal(size=(10, 1))
    v = rng.normal(size=(10, 1))
    v[3, 0] = math.nan
    with pytest.raises(Exp2Error, match="NaN or Inf"):
        ksg_mi(u, v, k=3)


def test_ksg_raises_on_duplicate_rows_without_jitter_dim_1():
    """KSG is undefined on duplicate joint rows; must raise unless jitter_seed given."""
    rng = np.random.default_rng(0)
    u = rng.normal(size=(20, 1))
    v = rng.normal(size=(20, 1))
    u[0] = u[1]
    v[0] = v[1]
    with pytest.raises(Exp2Error, match="duplicate rows"):
        ksg_mi(u, v, k=3, jitter_seed=None)


def test_ksg_raises_on_too_small_n():
    u = np.zeros((3, 4), dtype=np.float64)
    v = np.zeros((3, 4), dtype=np.float64)
    with pytest.raises(Exp2Error, match=r"N>=4"):
        ksg_mi(u, v, k=3)


def test_ksg_raises_on_invalid_k_dim_1():
    rng = np.random.default_rng(1)
    u = rng.normal(size=(10, 1))
    v = rng.normal(size=(10, 1))
    with pytest.raises(Exp2Error, match=r"k must satisfy"):
        ksg_mi(u, v, k=0)
    with pytest.raises(Exp2Error, match=r"k must satisfy"):
        ksg_mi(u, v, k=10)


# ---------------------------------------------------------------------------
# R3 verification — provenance helper contracts.
# ---------------------------------------------------------------------------


def test_run_estimators_provenance_helper_fails_on_missing_artefacts(tmp_path):
    """Missing provenance must block estimator execution."""
    from scripts.exp2.run_estimators import load_provenance

    with pytest.raises(Exp2Error, match="Corpus manifest missing"):
        load_provenance(
            cfg_path="scripts/exp2/exp2.yml",
            out_dir=tmp_path,
            pca_models_path=tmp_path / "pca_models.npz",
        )


def test_run_estimators_provenance_helper_requires_all_embedder_hashes(tmp_path):
    from scripts.exp2.run_estimators import embedder_weight_sha256, load_provenance
    from scripts.exp2.common import EMBEDDER_KEYS, canonical_json, sha256_bytes

    manifest = {
        "corpus_id": "exp2_corpus_v1",
        "version": "1.0.0",
        "total_inputs": 800,
        "strata": {f"S{i}": {"count": 100} for i in range(8)},
        "inputs": [],
    }
    manifest["manifest_sha256"] = sha256_bytes(canonical_json({k: v for k, v in manifest.items() if k != "manifest_sha256"}))
    (tmp_path / "corpus_manifest_v1.json").write_text(json.dumps(manifest, sort_keys=True))
    (tmp_path / "corpus_manifest_v1.json.sha256").write_text(manifest["manifest_sha256"] + "\n")
    np.savez_compressed(
        tmp_path / "pca_models.npz",
        metadata_json=np.asarray([json.dumps({key: {"sha256": "b" * 64} for key in EMBEDDER_KEYS})]),
    )
    (tmp_path / "verification_log.json").write_text(json.dumps({"embedder_hashes": {"e1_semantic": "a" * 64}}))
    with pytest.raises(Exp2Error, match="missing embedder hashes"):
        load_provenance("scripts/exp2/exp2.yml", tmp_path, tmp_path / "pca_models.npz")

    (tmp_path / "verification_log.json").write_text(
        json.dumps({"embedder_hashes": {key: "a" * 64 for key in EMBEDDER_KEYS}})
    )
    prov = load_provenance("scripts/exp2/exp2.yml", tmp_path, tmp_path / "pca_models.npz")
    assert embedder_weight_sha256("e1_semantic", prov["embedder_hashes"]) == "a" * 64
    assert embedder_weight_sha256("e9_hdc", prov["embedder_hashes"]) == "a" * 64


# ---------------------------------------------------------------------------
# R2 verification — orchestrator refuses incomplete master audit chains.
# ---------------------------------------------------------------------------


def test_orchestrator_finalize_master_audit_chain_fails_on_partial_run():
    """The master audit chain must not certify missing production artefacts."""
    import shutil
    import uuid

    from scripts.exp2.run_exp2 import finalize_master_audit_chain

    workspace = ARTEFACTS_DIR / f".test_master_chain_{uuid.uuid4().hex[:8]}"
    workspace.mkdir(parents=True, exist_ok=False)
    try:
        cfg_text = (
            f"run_id: exp2_test_partial\n"
            f"operator: pytest\n"
            # Path must be relative to repo for output_dir() to keep it under repo.
            f"output_dir: {workspace.relative_to(REPO_ROOT)}\n"
            f"run:\n  random_seed: 42\n"
        )
        cfg_path = workspace / "exp2_partial.yml"
        cfg_path.write_text(cfg_text)
        # Seed a phase chain file. Master chain hashes whatever is here.
        (workspace / "audit_chain_environment.json").write_text(
            json.dumps({"phase": "environment"})
        )

        with pytest.raises(Exp2Error, match="Required phase audit chain missing"):
            finalize_master_audit_chain(str(cfg_path))
    finally:
        shutil.rmtree(workspace, ignore_errors=True)
