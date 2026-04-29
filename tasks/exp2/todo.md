# EXP-2 Task Status

Source docs: `docs/exp2/06_atomic_tasks.md`.

| Task | Status | Evidence |
|---|---|---|
| T-001 Verify Environment | blocked | `artefacts/exp2/verification_log.json` now fails loudly: missing Python packages (`scipy`, `sklearn`, `torch`, `matplotlib`, `seaborn`) and RTX 5090 free VRAM 28,318 MiB < required 30,000 MiB. Cargo and all 14 embedder hashes are logged. |
| T-002 Build Per-Stratum Source Files | pending | Blocked by T-001. |
| T-003 Sample Exactly 100 Per Stratum | pending | Blocked by T-002. |
| T-004 Merge + Validate + Sign Manifest | pending | Blocked by T-003 and operator sign-off. |
| T-005 Build PCA Training Corpus | pending | Blocked by T-002. |
| T-006 Implement Embedding Extraction Script | completed | `scripts/exp2/extract_embeddings.py`. |
| T-007 Run Embedding Extraction | pending | Blocked by T-001. |
| T-008 PCA Training Corpus Embedding Extraction | pending | Blocked by T-001/T-005. |
| T-009 Fit PCA Models | implemented, not run | `scripts/exp2/fit_projections.py`. |
| T-010 Apply PCA To Audit Embeddings | implemented, not run | `scripts/exp2/fit_projections.py`. |
| T-011 Implement InfoNCE Estimator | completed | `scripts/exp2/estimators/infonce.py`; syntax and unit suite pass. |
| T-012 Implement KSG Estimator | completed | `scripts/exp2/estimators/ksg.py`; synthetic unit tests pass. |
| T-013 Synthetic Ground-Truth Validation | partially complete | Unit validation present; full report generation pending production-ready T-001. |
| T-014 Implement Bootstrap CI | completed | `scripts/exp2/estimators/bootstrap.py`; deterministic unit test passes. |
| T-015 Pilot Pass On 4 Pairs | pending | Blocked by T-007/T-010. |
| T-016 Run InfoNCE On 91 Pairs | implemented, not run | `scripts/exp2/run_estimators.py`; blocked by T-015. |
| T-017 Run KSG On 91 Pairs | implemented, not run | `scripts/exp2/run_estimators.py`; blocked by T-016. |
| T-018 Bootstrap All Pairs | implemented, not run | `scripts/exp2/run_estimators.py`; blocked by T-016/T-017. |
| T-019 Per-Embedder Entropy Estimation | implemented, not run | `scripts/exp2/effective_rank.py`; blocked by T-010. |
| T-020 Cross-Validation Metrics | implemented, not run | `scripts/exp2/run_estimators.py`; blocked by pair runs. |
| T-021 Compute Effective Rank | implemented, not run | `scripts/exp2/effective_rank.py`; blocked by T-018/T-019. |
| T-022 Render Visualizations | implemented, not run | `scripts/exp2/visualize.py`; blocked by T-021. |
| T-023 Per-Pair Recommendation Table | implemented, not run | `scripts/exp2/visualize.py`; blocked by T-021. |
| T-024 Audit Chain Finalization | implemented, blocked by artefacts | `scripts/exp2/run_exp2.py --phase audit` now refuses missing phase chains / artefacts. Current failure: missing `audit_chain_estimators.json`. |
| T-025 FSV Replay Test | implemented, not run | `scripts/exp2/fsv.py`, `scripts/exp2/replay.py`; blocked by production artefacts. |
| T-026 Operator Sign-Off On Headline | pending | Requires measured `n_eff_summary.json`. |
| T-027 Paper Patch Drafting | pending | Requires measured headline. |
| T-028 Pre-Register On OSF | pending | Requires operator account/action. |
| T-029 Open-Source Harness Release | pending | Requires completed harness artefacts and external release actions. |
| T-030 arXiv Revision Submission | pending | Requires paper patch and operator submission. |
| T-031 YouTube Video Production | pending | Requires measured heatmap/headline and operator publishing. |
| T-032 Blog Post | pending | Requires measured heatmap/headline and site publishing. |
| T-033 Tweet Thread | pending | Requires publication links. |
| T-100 Lessons File | completed | `tasks/exp2/lessons.md`. |
| T-101 Todo File | completed | This file. |

Latest verification:

- `pytest -q scripts/exp2/tests` -> 7 passed.
- `python3 scripts/exp2/manual_fsv.py --config scripts/exp2/exp2.yml` -> passed. Evidence: `artefacts/exp2/manual_fsv_log.json` read back 3 fingerprints and 3 training records, then proved 3 edge cases left source-of-truth counts unchanged.
- `python3 scripts/exp2/verify_environment.py --config scripts/exp2/exp2.yml` -> failed loudly until Python deps and GPU free memory are fixed. Evidence: `artefacts/exp2/verification_log.json`.
- `python3 scripts/exp2/build_corpus.py --config scripts/exp2/exp2.yml --phase sources` -> failed loudly because the required real S4 export is missing: `artefacts/exp2/learning_events_export.json`.
- `python3 scripts/exp2/fsv.py --config scripts/exp2/exp2.yml` -> failed loudly because production artefacts have not been generated (`embeddings_per_input.npz` is the first missing artefact).
