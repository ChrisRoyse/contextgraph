# Session 2026-05-11 — ME-JEPA forward-edge issues + doc ledger

## Scope

Continued after global `gh`/Infisical setup. The working objective was to return to the ME-JEPA/ruvector findings hardening task, file GitHub Issues for unresolved items called out by the docs, and update the docs so shipped work is not still described as pending.

## Issues filed / verified

Open issue ledger for unresolved forward-edge work:

- #3 neural `EmbedderForward` wrappers for active slots
- #4 analyzer/embedder-backed instrument heads
- #5 full SWE-bench Lite 300x8 corpus ship-gate FSV
- #6 converge or deprecate legacy `AstCodeChunker`
- #7 mutation operators beyond Python
- #8 supervised production scheduling for GC/subscriber/eval
- #9 production chunk-level TCT centroid refresh evidence
- #10 WSL GPU telemetry hardening
- #11 production calibration/OOD/latency telemetry
- #12 reformat-invariant AST chunk-ID audit
- #13 UTML formula-carrier reconciliation

Security issue #2 remains open for token rotation from the earlier pasted-token incident.

## Docs updated

Updated high-traffic status docs so they point at the issue ledger and no longer say shipped work is pending:

- `docs/ruvectorfindings/{00_INDEX,06_RISKS_AND_NON_TRANSFERABLE,09_END_GOAL_REVIEW_REPLACEMENT,10_MEJEPA_PIVOT_2026_05_09,12_EMBEDDER_INTEGRATION,14_AST_AWARE_CHUNKING_FOR_EMBEDDERS,15_PHASE_1_5_HARDENING_FSV_2026_05_10,16_PHASE_6_8_HARDENING_FSV_2026_05_11}.md`
- `docs/ruvectorfindings/specs/{_README,implementation-plan}.md`
- `docs/ruvectorfindings/specs/functional/{_index,SPEC-AST-CHUNKING,SPEC-UTML}.md`
- `docs/ruvectorfindings/specs/technical/{_index,TECH-AST-CHUNKING,TECH-UTML}.md`
- `docs/ruvectorfindings/specs/tasks/phase4-tct-constellation/_index.md`
- `docs/ruvectorfindings/specs/tasks/utml-learning-formula/{_index,TASK-UTML-002,TASK-UTML-003,TASK-UTML-004,TASK-UTML-005,TASK-UTML-007,TASK-UTML-008,TASK-UTML-009,TASK-UTML-010}.md`

Main corrections:

- Live ME-JEPA MCP surface is now 19 tools, including `mejepa_audit_pairwise_mi`.
- Pairwise-MI train-cert columns and MCP CSV/readback are shipped.
- Continual-backprop is wired into `heal/regulate.rs` and persists to `CF_MEJEPA_WEIGHT_BLOBS` with readback.
- Chunk-level TCT localization is code-shipped through `gtau_check_chunks`; only production-scale centroid/UX evidence remains.
- Legacy `AstCodeChunker`, reformat-invariant chunk IDs, production scheduling, production telemetry, full-corpus ship gate, and formula-carrier reconciliation are issue-tracked.

## Verification already run this session

Relevant FSV/physical readback evidence from the implementation pass:

- `cargo test -p context-graph-mcp pairwise_mi_audit --lib`
  - Source of truth: `/mnt/d/contextgraph/fsv/mcp-pairwise-mi-unit/audit/utml-pairwise-mi-matrix-00000042.csv`
  - CSV was mode 0600 and reloaded before tool return.
- `cargo run -p context-graph-mejepa --example continual_backprop_regulation_fsv`
  - Source of truth: `/mnt/d/contextgraph/fsv/contextgraph-mejepa-continual-backprop-fsv/continual_backprop_regulation_fsv_evidence.json`
  - RocksDB `CF_MEJEPA_WEIGHT_BLOBS` scan showed key `continual_backprop/7/0`; `CF_MEJEPA_TRAIN_CERTS` scan showed seeded cert rows.
- `cargo run -p context-graph-mejepa-train --example train_smoke_fsv`
  - Source of truth: `/tmp/contextgraph-mejepa-train-fsv/training-smoke-evidence.json`
  - RocksDB `CF_MEJEPA_TRAIN_CERTS` count was 50; first cert row contains `predictor_redundancy_pairwise_mi` and source.
- `cargo run -p context-graph-mejepa-train --example train_edge_cases_fsv`
  - Source of truth: `/tmp/contextgraph-mejepa-train-fsv/edge-case-evidence.json`
  - Verified target-collapse rejection before write, tamper detection, and empty holdout rejection.
- `cargo run -p context-graph-mejepa-train --example train_cert_chain_fsv`
  - Source of truth: `/tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json`
  - Verified 100000 certs and RocksDB count.

Doc-only follow-up verification:

- `gh issue list --limit 15 --json number,title,url,state` showed #3-#13 open with the expected titles.
- `rg` scans were run to remove stale high-level claims such as 18-tool MCP surface, pending pairwise-MI MCP wrapper, pending `heal/regulate.rs` continual-BP hook, and pending chunk-level TCT integration. Remaining "pending" language is intentional issue-tracked work.

## Notes

- `docs/futurebuild/*.md` were already untracked in the working tree during this pass and were not touched.
- This was a documentation/issue-ledger continuation after the implementation and FSV work; no Rust source changes were made in this pass.
