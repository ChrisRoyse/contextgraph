from __future__ import annotations

import numpy as np

from scripts.exp2.effective_rank import (
    n_eff_redundancy_adjusted,
    n_eff_schur_complement,
    n_eff_spectrum_entropy,
)


def test_redundancy_adjusted_count_known_three_embedder_cases() -> None:
    entropy = np.array([2.0, 2.0, 2.0])
    independent = np.diag(entropy)
    assert n_eff_redundancy_adjusted(independent, entropy) == 6.0
    fully_redundant = np.full((3, 3), 2.0)
    assert n_eff_redundancy_adjusted(fully_redundant, entropy) == 3.0


def test_spectrum_and_schur_are_finite_on_positive_matrix() -> None:
    matrix = np.array([[2.0, 0.2, 0.1], [0.2, 1.5, 0.3], [0.1, 0.3, 1.0]])
    spectrum, eigenvalues = n_eff_spectrum_entropy(matrix)
    assert 1.0 <= spectrum <= 3.0
    assert len(eigenvalues) == 3
    assert n_eff_schur_complement(matrix, 0.05) == 3
