# E6 Sparse Embedder Benchmark Report

Generated: 2026-01-22 21:34:25 UTC

## Overview

This benchmark evaluates the E6 (V_selectivity) sparse embedder using real Wikipedia data.
E6 uses keyword-based sparse representations for exact term matching.

## Summary

| Metric | Value | Target | Status |
|--------|-------|--------|--------|
| E6 MRR@10 | 0.6844 | >= 0.50 | PASS |
| E6 vs E1 Delta | -0.0078 | > 0.00 | FAIL |
| Sparsity | 0.9923 | > 0.95 | PASS |

## Retrieval Quality

| Embedder | MRR@10 | Notes |
|----------|--------|-------|
| E6 Sparse | 0.6844 | Keyword-based sparse |
| E1 Semantic | 0.6922 | Dense baseline |
| E13 SPLADE | 0.6747 | Learned sparse |

## Sparsity Analysis

- **Average Active Terms**: 234.6
- **Sparsity Ratio**: 0.9923 (99.23% zeros)
- **Vocabulary Size**: 30522 (BERT tokenizer)

## Dataset

- **Corpus Size**: 5000 documents
- **Query Count**: 93 queries
- **Topics**: 52 unique topics

## Performance

- **Data Loading**: 9ms
- **Embedding**: 3863299ms
- **Evaluation**: 3687ms
- **Total**: 3867000ms

## Per-Topic Performance

| Topic | MRR@10 |
|-------|--------|
| amphibian | 1.0000 |
| alberta | 1.0000 |
| asparagales | 1.0000 |
| answer | 1.0000 |
| animation | 1.0000 |
| asteroids | 1.0000 |
| anatomy | 1.0000 |
| algorithm | 1.0000 |
| aruba | 1.0000 |
| andre | 1.0000 |
| annual | 1.0000 |
| android | 1.0000 |
| art | 1.0000 |
| agnostida | 1.0000 |
| austrian | 1.0000 |

