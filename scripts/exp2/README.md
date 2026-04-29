# EXP-2 Harness

Strict pairwise mutual-information audit harness for `docs/exp2`.

Run order:

```bash
python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml
python3 scripts/exp2/build_corpus.py --config scripts/exp2/exp2.yml --phase all --learning-events-json <real_cf_learning_events_export.json>
python3 scripts/exp2/extract_embeddings.py --config scripts/exp2/exp2.yml
python3 scripts/exp2/extract_embeddings.py --config scripts/exp2/exp2.yml --manifest artefacts/exp2/pca_training_corpus_v1.json --output artefacts/exp2/pca_training_embeddings.npz --storage-name pca
python3 scripts/exp2/fit_projections.py --config scripts/exp2/exp2.yml --fit --apply
python3 scripts/exp2/run_estimators.py --config scripts/exp2/exp2.yml
python3 scripts/exp2/effective_rank.py --config scripts/exp2/exp2.yml
python3 scripts/exp2/visualize.py --config scripts/exp2/exp2.yml
python3 scripts/exp2/fsv.py --config scripts/exp2/exp2.yml
```

The extraction path uses the real MCP server and verifies persisted state by reading back `CF_FINGERPRINTS` and `CF_TRAINING_RECORDS`.
