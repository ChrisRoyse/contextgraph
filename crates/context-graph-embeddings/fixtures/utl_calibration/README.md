# UTL Calibration Fixtures

This fixture set is the checked-in calibration source for the local E15-E21
learner-state embedder verification path. The Rust fixture generator
`context_graph_embeddings::learner::synthetic_calibration_fixture()` is the
executable source of truth; these JSON files document the same expected shape
for manual inspection and external tooling.

The public/research datasets in `dataset_manifest.json` are calibration targets
for real model tuning. They are not vendored because most require explicit
research access or license review.
