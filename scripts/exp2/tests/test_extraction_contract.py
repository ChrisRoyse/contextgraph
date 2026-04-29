from __future__ import annotations

import numpy as np

from scripts.exp2.extract_embeddings import token_mean, vector_from_record


def training_record_fixture() -> dict:
    return {
        "e1_semantic": [1.0] * 1024,
        "e2_temporal_recent": [1.0] * 512,
        "e3_temporal_periodic": [1.0] * 512,
        "e4_temporal_positional": [1.0] * 512,
        "e5_causal_cause": [1.0] * 768,
        "e7_code": [1.0] * 1536,
        "e8_graph_source": [1.0] * 1024,
        "e9_hdc": [1.0] * 1024,
        "e10_paraphrase": [1.0] * 768,
        "e11_entity": [1.0] * 768,
        "e14_bge_m3_dense": [1.0] * 1024,
        "e6_sparse_indices": list(range(130)),
        "e6_sparse_values": [float(i + 1) for i in range(130)],
        "e13_splade_indices": list(range(130)),
        "e13_splade_values": [float(i + 1) for i in range(130)],
        "e12_token_embeddings": [[1.0] * 128, [3.0] * 128],
    }


def test_training_record_vector_contract() -> None:
    rec = training_record_fixture()
    assert vector_from_record(rec, "e1_semantic").shape == (1024,)
    assert vector_from_record(rec, "e6_sparse_top_k").shape == (128,)
    assert vector_from_record(rec, "e13_splade_top_k").shape == (128,)
    pooled = token_mean(rec)
    assert pooled.shape == (128,)
    assert np.allclose(pooled, 2.0)
